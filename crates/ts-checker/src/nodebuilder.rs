use ts_collections::FastHashMap as HashMap;
pub use ts_nodebuilder::*;
use ts_printer as printer;

use crate::checker::*;
use crate::nodebuilderimpl::{SignatureToSignatureDeclarationOptions, new_node_builder_impl_owned};
use crate::semantic::SymbolIdentity;
use crate::{ast, core, nodebuilder};

pub struct NodeBuilder<'a, 'state, 'c, 'e> {
    ctx_stack: Vec<NodeBuilderContext<'a>>,
    tracker_stack: Vec<Option<Box<dyn nodebuilder::SymbolTracker + 'a>>>,
    host: &'a dyn Host,
    impl_: NodeBuilderImpl<'a, 'state, 'c, 'e>,
    verbosity_level: i32,
    verbosity_max_truncation_length: i32,
    verbosity_can_increase: bool,
    verbosity_truncated: bool,
}

// VerbosityContext controls hover-expansion behavior in the node builder.
// A nil VerbosityContext means no expansion (non-hover callers).
// Level 0 = default hover (maxExpansionDepth = 0; detects expandability without expanding).
// Level 1+ = expansion enabled (maxExpansionDepth = Level).
pub struct VerbosityContext {
    pub level: i32,                   // 0 = default (no expansion), 1+ = expansion depth
    pub max_truncation_length: i32,   // 0 = use default
    pub can_increase_verbosity: bool, // output: whether increasing Level would reveal more
    pub truncated: bool,              // output: whether output was truncated
}

impl<'a, 'state, 'c, 'e> NodeBuilder<'a, 'state, 'c, 'e> {
    pub fn store(&self) -> &ast::AstStore {
        self.impl_.e.factory.node_factory.store()
    }

    pub fn id_to_symbol_identities(&self) -> HashMap<ast::IdentifierNode, SymbolIdentity> {
        self.impl_.id_to_symbol.clone()
    }

    pub(crate) fn set_verbosity(&mut self, verbosity: &VerbosityContext) {
        self.verbosity_level = verbosity.level;
        self.verbosity_max_truncation_length = verbosity.max_truncation_length;
    }

    pub(crate) fn write_verbosity(&self, verbosity: &mut VerbosityContext) {
        if self.verbosity_can_increase {
            verbosity.can_increase_verbosity = true;
        }
        if self.verbosity_truncated {
            verbosity.truncated = true;
        }
    }

    // EmitContext implements NodeBuilderInterface.
    pub fn emit_context(&mut self) -> &mut printer::EmitContext {
        &mut self.impl_.e
    }

    pub(crate) fn prepare_node_for_emit(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.impl_.e.factory.node_factory.store().store_id() {
            return node;
        }
        node
    }

    pub(crate) fn prepare_optional_node_for_emit(
        &mut self,
        node: Option<ast::Node>,
    ) -> Option<ast::Node> {
        node.map(|node| self.prepare_node_for_emit(node))
    }

    pub(crate) fn prepare_nodes_for_emit(&mut self, nodes: Vec<ast::Node>) -> Vec<ast::Node> {
        nodes
            .into_iter()
            .map(|node| self.prepare_node_for_emit(node))
            .collect()
    }

    fn enter_context(
        &mut self,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) {
        let enclosing_file = enclosing_declaration.and_then(|declaration| {
            self.impl_
                .source_file_identity_for_enclosing_declaration(declaration)
        });
        let new_ctx = NodeBuilderContext {
            host: self.host,
            approximate_length: 0,
            flags,
            internal_flags,
            depth: 0,
            max_expansion_depth: self.verbosity_level,
            max_truncation_length: self.verbosity_max_truncation_length as usize,
            enclosing_declaration,
            enclosing_file,
            infer_type_parameters: Vec::new(),
            visited_types: Default::default(),
            symbol_depth: Default::default(),
            tracked_symbols: Vec::new(),
            mapper: None,
            reverse_mapped_stack: Vec::new(),
            enclosing_symbol_types: Default::default(),
            remapped_symbol_references: Default::default(),
            suppress_report_inference_fallback: false,
            encountered_error: false,
            truncating: false,
            reported_diagnostic: false,
            type_stack: Vec::new(),
            can_increase_expansion_depth: false,
            expansion_truncated: false,
            type_parameter_names: Default::default(),
            type_parameter_names_by_text: Default::default(),
            type_parameter_names_by_text_next_name_count: Default::default(),
            type_parameter_symbol_list: Default::default(),
        };
        let old_ctx = std::mem::replace(&mut self.impl_.ctx, new_ctx);
        self.ctx_stack.push(old_ctx);
        let old_tracker = std::mem::replace(&mut self.impl_.tracker, tracker);
        self.tracker_stack.push(old_tracker);
    }

    // propagateVerbosityOut copies expansion signals from the context to the VerbosityContext output.
    fn propagate_verbosity_out(&mut self) {
        if self.impl_.ctx.can_increase_expansion_depth {
            self.verbosity_can_increase = true;
        }
        if self.impl_.ctx.expansion_truncated {
            self.verbosity_truncated = true;
        }
    }

    fn pop_context(&mut self) {
        if let Some(ctx) = self.ctx_stack.pop() {
            self.impl_.ctx = ctx;
            self.impl_.tracker = self.tracker_stack.pop().unwrap_or(None);
        } else {
            panic!("node builder context stack underflow");
        }
    }

    fn exit_context(&mut self, result: Option<ast::Node>) -> Option<ast::Node> {
        self.propagate_verbosity_out();
        self.exit_context_check();
        let encountered_error = self.impl_.ctx.encountered_error;
        self.pop_context();
        if encountered_error {
            return None;
        }
        result
    }

    fn exit_context_slice(&mut self, result: Vec<ast::Node>) -> Vec<ast::Node> {
        self.propagate_verbosity_out();
        self.exit_context_check();
        let encountered_error = self.impl_.ctx.encountered_error;
        self.pop_context();
        if encountered_error {
            return Vec::new();
        }
        result
    }

    fn exit_context_check(&mut self) {
        if self.impl_.ctx.truncating && self.impl_.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION != 0
        {
            self.impl_.report_truncation_error();
        }
    }

    // IndexInfoToIndexSignatureDeclaration implements NodeBuilderInterface.
    pub fn index_info_to_index_signature_declaration(
        &mut self,
        info: IndexInfoHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self
            .impl_
            .index_info_to_index_signature_declaration_helper(info, None);
        self.exit_context(Some(result))
    }

    // SerializeReturnTypeForSignature implements NodeBuilderInterface.
    pub fn serialize_return_type_for_signature(
        &mut self,
        signature_declaration: ast::Node,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let signature = self
            .impl_
            .ch
            .get_signature_from_declaration(signature_declaration);
        let (_, cleanup) = self.impl_.enter_signature_scope(signature);
        let result = self
            .impl_
            .serialize_return_type_for_signature(signature, true);
        self.impl_.exit_scope(cleanup);
        self.exit_context(result)
    }

    pub fn serialize_type_parameters_for_signature(
        &mut self,
        signature_declaration: ast::Node,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Vec<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let symbol = self
            .impl_
            .ch
            .get_symbol_of_declaration(signature_declaration);
        let Some(symbol) = symbol else {
            return self.exit_context_slice(Vec::new());
        };
        let type_params = self.symbol_to_type_parameter_declarations_handle(
            symbol,
            enclosing_declaration,
            flags,
            internal_flags,
            None,
        );
        self.exit_context_slice(type_params)
    }

    // SerializeTypeForDeclaration implements NodeBuilderInterface.
    pub fn serialize_type_for_declaration(
        &mut self,
        declaration: ast::Node,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self
            .impl_
            .serialize_type_for_declaration_for_symbol_identity(
                Some(declaration),
                None,
                Some(symbol),
                true,
            );
        self.exit_context(Some(result))
    }

    pub fn get_declaration_statements_for_source_file(
        &mut self,
        source_file: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Vec<ast::Node> {
        self.enter_context(Some(source_file), flags, internal_flags, tracker);
        let result = self
            .impl_
            .get_declaration_statements_for_source_file(source_file);
        self.exit_context_slice(result)
    }

    // SerializeTypeForExpression implements NodeBuilderInterface.
    pub fn serialize_type_for_expression(
        &mut self,
        expr: ast::Node,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.serialize_type_for_expression(expr);
        self.exit_context(Some(result))
    }

    // SignatureToSignatureDeclaration implements NodeBuilderInterface.
    pub fn signature_to_signature_declaration(
        &mut self,
        signature: SignatureHandle,
        kind: ast::Kind,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self
            .impl_
            .signature_to_signature_declaration_helper(signature, kind, None);
        self.exit_context(Some(result))
    }

    pub fn signature_to_signature_declaration_with_options(
        &mut self,
        signature: SignatureHandle,
        kind: ast::Kind,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
        modifiers: Vec<ast::Node>,
        name: Option<ast::Node>,
        question_token: Option<ast::Node>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.signature_to_signature_declaration_helper(
            signature,
            kind,
            Some(SignatureToSignatureDeclarationOptions {
                modifiers,
                name,
                question_token,
            }),
        );
        self.exit_context(Some(result))
    }

    // ExpandSymbolForHover produces declaration nodes for a symbol with verbosity level support.
    pub(crate) fn expand_symbol_identity_for_hover(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
    ) -> Vec<ast::Node> {
        self.enter_context(
            None,
            nodebuilder::FLAGS_IGNORE_ERRORS
                | nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS
                | nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE,
            nodebuilder::INTERNAL_FLAGS_NONE,
            None,
        );

        // Push the declared type onto the type stack to prevent re-expansion.
        // We push a nil sentinel after the real type so that isTypeOnStack
        // (which skips the last element) still checks declaredType.
        let declared_type = self
            .impl_
            .ch
            .get_declared_type_of_symbol_identity_or_error(symbol);
        self.impl_
            .ctx
            .type_stack
            .push(Some(self.impl_.ch.type_id(declared_type)));
        self.impl_.ctx.type_stack.push(None);

        let nodes = self.impl_.expand_symbol_for_hover(symbol);

        let len = self.impl_.ctx.type_stack.len();
        self.impl_.ctx.type_stack.truncate(len - 2);

        self.propagate_verbosity_out();

        // Simplify declarations by applying original modifiers
        let mut result = Vec::with_capacity(nodes.len());
        for node in nodes {
            match self.impl_.e.factory.node_factory.store().kind(node) {
                ast::KIND_CLASS_DECLARATION => result.push(simplify_class_declaration(
                    self.impl_.ch,
                    &mut self.impl_.e.factory.node_factory,
                    node,
                    symbol,
                )),
                ast::KIND_ENUM_DECLARATION => result.push(simplify_modifiers(
                    self.impl_.ch,
                    &mut self.impl_.e.factory.node_factory,
                    node,
                    ast::is_enum_declaration,
                    symbol,
                )),
                ast::KIND_INTERFACE_DECLARATION => {
                    if meaning & ast::SYMBOL_FLAGS_INTERFACE != 0 {
                        result.push(simplify_modifiers(
                            self.impl_.ch,
                            &mut self.impl_.e.factory.node_factory,
                            node,
                            ast::is_interface_declaration,
                            symbol,
                        ));
                    }
                }
                ast::KIND_MODULE_DECLARATION => result.push(simplify_modifiers(
                    self.impl_.ch,
                    &mut self.impl_.e.factory.node_factory,
                    node,
                    ast::is_module_declaration,
                    symbol,
                )),
                _ => {}
            }
        }

        self.exit_context_slice(result)
    }

    // SymbolToEntityName implements NodeBuilderInterface.
    pub fn symbol_to_entity_name(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.symbol_to_name_identity(symbol, meaning, false);
        self.exit_context(Some(result))
    }

    // SymbolToExpression implements NodeBuilderInterface.
    pub fn symbol_to_expression(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.symbol_to_expression(symbol, meaning);
        self.exit_context(Some(result))
    }

    // SymbolToNode implements NodeBuilderInterface.
    pub fn symbol_to_node(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.symbol_to_node(symbol, meaning);
        self.exit_context(Some(result))
    }

    // SymbolToParameterDeclaration implements NodeBuilderInterface.
    pub fn symbol_to_parameter_declaration(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.symbol_to_parameter_declaration(symbol, false);
        self.exit_context(Some(result))
    }

    // SymbolToTypeParameterDeclarations implements NodeBuilderInterface.
    pub fn symbol_to_type_parameter_declarations(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Vec<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self
            .impl_
            .symbol_to_type_parameter_declarations_identity(symbol);
        self.exit_context_slice(result)
    }

    pub fn symbol_to_type_parameter_declarations_handle(
        &mut self,
        symbol: ast::SymbolHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Vec<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self
            .impl_
            .symbol_to_type_parameter_declarations_handle(symbol);
        self.exit_context_slice(result)
    }

    // TypeParameterToDeclaration implements NodeBuilderInterface.
    pub fn type_parameter_to_declaration(
        &mut self,
        parameter: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.type_parameter_to_declaration(parameter);
        self.exit_context(Some(result))
    }

    // TypePredicateToTypePredicateNode implements NodeBuilderInterface.
    pub fn type_predicate_to_type_predicate_node(
        &mut self,
        predicate: TypePredicateHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.type_predicate_to_type_predicate_node(predicate);
        self.exit_context(Some(result))
    }

    // TypeToTypeNode implements NodeBuilderInterface.
    pub fn type_to_type_node(
        &mut self,
        typ: TypeHandle,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.type_to_type_node(typ);
        self.exit_context(result)
    }

    pub fn try_js_type_node_to_type_node(
        &mut self,
        node: ast::Node,
        enclosing_declaration: Option<ast::Node>,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,
    ) -> Option<ast::Node> {
        self.enter_context(enclosing_declaration, flags, internal_flags, tracker);
        let result = self.impl_.try_js_type_node_to_type_node(Some(node));
        self.exit_context(result)
    }
}

fn simplify_class_declaration<'a>(
    ch: &Checker<'a, '_>,
    f: &mut ast::NodeFactory,
    class_decl: ast::Node,
    symbol: SymbolIdentity,
) -> ast::Node {
    let class_declarations = ch
        .collect_symbol_identity_declarations(symbol)
        .into_iter()
        .filter(|declaration| ast::is_class_like(ch.store_for_node(*declaration), *declaration))
        .collect::<Vec<_>>();
    let original_class_decl = if !class_declarations.is_empty() {
        class_declarations[0]
    } else {
        class_decl
    };
    let original_store = ch.store_for_node(original_class_decl);
    let modifiers = ast::get_combined_modifier_flags(original_store, original_class_decl)
        & !(ast::MODIFIER_FLAGS_EXPORT | ast::MODIFIER_FLAGS_AMBIENT);
    let is_anonymous = ast::is_class_expression(original_store, original_class_decl);
    let class_decl = if is_anonymous {
        if ast::is_class_declaration(f.store(), class_decl) {
            f.clear_synthetic_class_declaration_name(class_decl);
        }
        class_decl
    } else {
        class_decl
    };
    let modifier_nodes = create_modifiers_from_modifier_flags(f, modifiers);
    let modifier_list = f.new_modifier_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        modifier_nodes,
        modifiers,
    );
    let replaced = ast::replace_modifiers(f, class_decl, Some(modifier_list));
    replaced
}

fn simplify_modifiers<'a>(
    ch: &Checker<'a, '_>,
    f: &mut ast::NodeFactory,
    new_decl: ast::Node,
    is_decl_kind: fn(&ast::AstStore, ast::Node) -> bool,
    symbol: SymbolIdentity,
) -> ast::Node {
    let decls = ch
        .collect_symbol_identity_declarations(symbol)
        .into_iter()
        .filter(|d| is_decl_kind(ch.store_for_node(*d), *d))
        .collect::<Vec<_>>();
    let decl_with_modifiers = if !decls.is_empty() {
        decls[0]
    } else {
        new_decl
    };
    let decl_store = ch.store_for_node(decl_with_modifiers);
    let modifiers = ast::get_combined_modifier_flags(decl_store, decl_with_modifiers)
        & !(ast::MODIFIER_FLAGS_EXPORT | ast::MODIFIER_FLAGS_AMBIENT);
    let modifier_nodes = create_modifiers_from_modifier_flags(f, modifiers);
    let modifier_list = f.new_modifier_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        modifier_nodes,
        modifiers,
    );
    let replaced = ast::replace_modifiers(f, new_decl, Some(modifier_list));
    replaced
}

fn create_modifiers_from_modifier_flags(
    f: &mut ast::NodeFactory,
    flags: ast::ModifierFlags,
) -> Vec<ast::Node> {
    let mut result = Vec::new();
    if (flags & ast::MODIFIER_FLAGS_EXPORT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ExportKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_AMBIENT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::DeclareKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_DEFAULT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::DefaultKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_CONST) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ConstKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_PUBLIC) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::PublicKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_PRIVATE) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::PrivateKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_PROTECTED) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ProtectedKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_ABSTRACT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::AbstractKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_STATIC) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::StaticKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_OVERRIDE) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::OverrideKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_READONLY) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::ReadonlyKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_ACCESSOR) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::AccessorKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_ASYNC) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::AsyncKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_IN) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::InKeyword));
    }
    if (flags & ast::MODIFIER_FLAGS_OUT) != ast::MODIFIER_FLAGS_NONE {
        result.push(f.new_modifier(ast::Kind::OutKeyword));
    }
    result
}

// var _ NodeBuilderInterface = NewNodeBuilderAPI(nil, nil)

pub fn new_node_builder<'a, 'state, 'c, 'e>(
    ch: &'c mut Checker<'a, 'state>,
    e: &'e mut printer::EmitContext,
) -> NodeBuilder<'a, 'state, 'c, 'e> {
    new_node_builder_ex(ch, e, None /*idToSymbol*/)
}

pub fn new_node_builder_ex<'a, 'state, 'c, 'e>(
    ch: &'c mut Checker<'a, 'state>,
    e: &'e mut printer::EmitContext,
    id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
) -> NodeBuilder<'a, 'state, 'c, 'e> {
    let host = ch.program;
    let impl_ = new_node_builder_impl(ch, e, id_to_symbol);
    NodeBuilder {
        impl_,
        ctx_stack: Vec::with_capacity(1),
        tracker_stack: Vec::with_capacity(1),
        host,
        verbosity_level: -1,
        verbosity_max_truncation_length: 0,
        verbosity_can_increase: false,
        verbosity_truncated: false,
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn get_node_builder<'c>(&'c mut self) -> NodeBuilder<'a, 'state, 'c, 'c> {
        self.get_node_builder_ex(None /*idToSymbol*/)
    }

    pub(crate) fn get_node_builder_ex<'c>(
        &'c mut self,
        id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
    ) -> NodeBuilder<'a, 'state, 'c, 'c> {
        let host = self.program;
        let impl_ = new_node_builder_impl_owned(self, printer::new_emit_context(), id_to_symbol);
        NodeBuilder {
            impl_,
            ctx_stack: Vec::with_capacity(1),
            tracker_stack: Vec::with_capacity(1),
            host,
            verbosity_level: -1,
            verbosity_max_truncation_length: 0,
            verbosity_can_increase: false,
            verbosity_truncated: false,
        }
    }
}
