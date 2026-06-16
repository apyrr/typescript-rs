// package checker

use std::cell::RefCell;
use std::ops::ControlFlow;
use std::rc::Rc;

use ts_ast as ast;
use ts_binder as binder;
use ts_collections::{
    FastHashMap as HashMap, FastHashMapExt, FastHashSet as HashSet, FastHashSetExt,
};
use ts_core as core;
use ts_evaluator as evaluator;
use ts_modulespecifiers::CheckerShape;
use ts_nodebuilder as nodebuilder;
use ts_parser as parser;
use ts_printer as printer;

use crate::checker::*;
use crate::nodebuilder::new_node_builder;

struct SharedSymbolTracker {
    inner: Rc<RefCell<Box<dyn nodebuilder::SymbolTracker>>>,
}

impl nodebuilder::SymbolTracker for SharedSymbolTracker {
    fn track_symbol(
        &mut self,
        symbol: ast::SymbolIdentity,
        symbol_flags: ast::SymbolFlags,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        self.inner
            .borrow_mut()
            .track_symbol(symbol, symbol_flags, enclosing_declaration, meaning)
    }

    fn report_inaccessible_this_error(&mut self) {
        self.inner.borrow_mut().report_inaccessible_this_error();
    }

    fn report_private_in_base_of_class_expression(&mut self, property_name: &str) {
        self.inner
            .borrow_mut()
            .report_private_in_base_of_class_expression(property_name);
    }

    fn report_inaccessible_unique_symbol_error(&mut self) {
        self.inner
            .borrow_mut()
            .report_inaccessible_unique_symbol_error();
    }

    fn report_cyclic_structure_error(&mut self) {
        self.inner.borrow_mut().report_cyclic_structure_error();
    }

    fn report_likely_unsafe_import_required_error(&mut self, specifier: &str, symbol_name: &str) {
        self.inner
            .borrow_mut()
            .report_likely_unsafe_import_required_error(specifier, symbol_name);
    }

    fn report_truncation_error(&mut self) {
        self.inner.borrow_mut().report_truncation_error();
    }

    fn report_nonlocal_augmentation(
        &mut self,
        containing_file: &ast::SourceFile,
        parent_symbol: ast::SymbolIdentity,
        augmenting_symbol: ast::SymbolIdentity,
    ) {
        self.inner.borrow_mut().report_nonlocal_augmentation(
            containing_file,
            parent_symbol,
            augmenting_symbol,
        );
    }

    fn report_non_serializable_property(&mut self, property_name: &str) {
        self.inner
            .borrow_mut()
            .report_non_serializable_property(property_name);
    }

    fn mark_aliases_visible(&mut self, aliases: &[ast::Node]) {
        self.inner.borrow_mut().mark_aliases_visible(aliases);
    }

    fn report_symbol_accessibility_error(
        &mut self,
        accessibility: nodebuilder::SymbolAccessibility,
        error_symbol_name: &str,
        error_module_name: &str,
        error_node: Option<ast::Node>,
    ) -> bool {
        self.inner.borrow_mut().report_symbol_accessibility_error(
            accessibility,
            error_symbol_name,
            error_module_name,
            error_node,
        )
    }

    fn report_inference_fallback(&mut self, node: ast::Node) {
        self.inner.borrow_mut().report_inference_fallback(node);
    }

    fn push_error_fallback_node(&mut self, node: ast::Node) {
        self.inner.borrow_mut().push_error_fallback_node(node);
    }

    fn pop_error_fallback_node(&mut self) {
        self.inner.borrow_mut().pop_error_fallback_node();
    }
}

// Links for jsx
#[derive(Default)]
pub struct JSXLinks {
    import_ref: Option<ast::Node>,
}

impl JSXLinks {
    pub(crate) fn import_ref(&self) -> Option<ast::Node> {
        self.import_ref
    }

    pub(crate) fn set_import_ref(&mut self, import_ref: Option<ast::Node>) {
        self.import_ref = import_ref;
    }
}

pub(crate) trait JSXLinksStoreExt {
    fn jsx_link_handle(&self, node: ast::Node) -> core::LinkHandle<JSXLinks>;
    fn jsx_import_ref(&self, node: ast::Node) -> Option<ast::Node>;
    fn jsx_import_ref_by_handle(&self, handle: core::LinkHandle<JSXLinks>) -> Option<ast::Node>;
    fn set_jsx_import_ref(&self, node: ast::Node, import_ref: Option<ast::Node>);
    fn set_jsx_import_ref_by_handle(
        &self,
        handle: core::LinkHandle<JSXLinks>,
        import_ref: Option<ast::Node>,
    );
}

// Links for declarations

#[derive(Default)]
pub struct DeclarationLinks {
    is_visible: core::Tristate, // if declaration is depended upon by exported declarations
}

impl DeclarationLinks {
    pub(crate) fn is_visible(&self) -> core::Tristate {
        self.is_visible
    }

    pub(crate) fn set_is_visible(&mut self, value: core::Tristate) {
        self.is_visible = value;
    }
}

#[derive(Default)]
pub struct DeclarationFileLinks {
    aliases_marked: bool, // if file has had alias visibility marked
}

impl DeclarationFileLinks {
    pub(crate) fn aliases_marked(&self) -> bool {
        self.aliases_marked
    }

    pub(crate) fn set_aliases_marked(&mut self, value: bool) {
        self.aliases_marked = value;
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn with_symbol_identity_declarations<R>(
        &self,
        symbol: SymbolIdentity,
        f: impl FnOnce(&[ast::Node]) -> R,
    ) -> R {
        self.with_symbol_handle_declarations(symbol.symbol_handle(), f)
    }

    pub(crate) fn collect_symbol_identity_declarations(
        &self,
        symbol: SymbolIdentity,
    ) -> Vec<ast::Node> {
        self.collect_symbol_handle_declarations(symbol.symbol_handle())
    }

    pub(crate) fn first_symbol_identity_declaration(
        &self,
        symbol: SymbolIdentity,
    ) -> Option<ast::Node> {
        self.first_symbol_handle_declaration(symbol.symbol_handle())
    }

    pub(crate) fn symbol_identity_declarations_are_empty(&self, symbol: SymbolIdentity) -> bool {
        self.symbol_handle_declarations_are_empty(symbol.symbol_handle())
    }

    pub(crate) fn symbol_identity_declaration_count(&self, symbol: SymbolIdentity) -> usize {
        self.with_symbol_identity_declarations(symbol, |declarations| declarations.len())
    }

    pub(crate) fn all_symbol_identity_declarations(
        &self,
        symbol: SymbolIdentity,
        predicate: impl FnMut(ast::Node) -> bool,
    ) -> bool {
        self.all_symbol_handle_declarations(symbol.symbol_handle(), predicate)
    }

    pub(crate) fn for_each_symbol_identity_declaration(
        &mut self,
        symbol: SymbolIdentity,
        f: impl FnMut(&mut Self, ast::Node),
    ) {
        self.for_each_symbol_handle_declaration(symbol.symbol_handle(), f);
    }

    pub(crate) fn any_symbol_identity_declaration(
        &mut self,
        symbol: SymbolIdentity,
        predicate: impl FnMut(&mut Self, ast::Node) -> bool,
    ) -> bool {
        self.any_symbol_handle_declaration(symbol.symbol_handle(), predicate)
    }

    pub(crate) fn find_symbol_identity_declaration(
        &mut self,
        symbol: SymbolIdentity,
        predicate: impl FnMut(&mut Self, ast::Node) -> bool,
    ) -> Option<ast::Node> {
        self.find_symbol_handle_declaration(symbol.symbol_handle(), predicate)
    }

    fn symbol_identity_flags_for_emit(&self, symbol: SymbolIdentity) -> ast::SymbolFlags {
        self.symbol_handle_flags(symbol.symbol_handle())
    }

    fn symbol_identity_value_declaration_for_emit(
        &self,
        symbol: SymbolIdentity,
    ) -> Option<ast::Node> {
        self.symbol_handle_value_declaration(symbol.symbol_handle())
    }

    fn symbol_identity_parent_for_emit(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        self.symbol_handle_parent(symbol.symbol_handle())
            .map(SymbolIdentity::from_symbol_handle)
    }

    fn symbol_identity_export_symbol_for_emit(
        &self,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        self.symbol_handle_export_symbol(symbol.symbol_handle())
            .map(SymbolIdentity::from_symbol_handle)
    }

    fn is_constant_variable_identity_for_emit(&mut self, symbol: SymbolIdentity) -> bool {
        let handle = symbol.symbol_handle();
        self.symbol_handle_flags(handle)
            .intersects(ast::SYMBOL_FLAGS_VARIABLE)
            && self
                .symbol_handle_value_declaration(handle)
                .is_some_and(|declaration| {
                    self.get_combined_node_flags_cached(declaration)
                        .intersects(ast::NODE_FLAGS_CONSTANT)
                })
    }

    fn is_const_enum_or_const_enum_only_module_identity_for_emit(
        &self,
        symbol: SymbolIdentity,
    ) -> bool {
        let handle = symbol.symbol_handle();
        let flags = self.symbol_handle_flags(handle);
        flags & ast::SYMBOL_FLAGS_CONST_ENUM != 0
            || flags & ast::SYMBOL_FLAGS_CONST_ENUM_ONLY_MODULE != 0
    }

    fn get_type_only_alias_declaration_identity_for_emit(
        &mut self,
        symbol: SymbolIdentity,
        include: ast::SymbolFlags,
    ) -> Option<ast::Node> {
        let _ = include;
        self.get_type_only_alias_declaration_handle(symbol.symbol_handle())
    }

    fn get_export_symbol_of_value_symbol_identity_if_exported_for_emit(
        &mut self,
        symbol: Option<SymbolIdentity>,
    ) -> Option<SymbolIdentity> {
        let symbol = symbol?;
        let handle = symbol.symbol_handle();
        let export_symbol = if self
            .symbol_handle_flags(handle)
            .intersects(ast::SYMBOL_FLAGS_EXPORT_VALUE)
        {
            self.symbol_handle_export_symbol(handle)
                .map(SymbolIdentity::from_symbol_handle)
        } else {
            None
        };
        self.get_merged_symbol_identity(export_symbol.or(Some(symbol)))
    }

    fn get_signatures_of_symbol_handle(
        &mut self,
        symbol: Option<ast::SymbolHandle>,
    ) -> Vec<SignatureHandle> {
        let Some(symbol) = symbol else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let declaration_count =
            self.with_symbol_handle_declarations(symbol, |declarations| declarations.len());
        for i in 0..declaration_count {
            let decl = self.with_symbol_handle_declarations(symbol, |declarations| declarations[i]);
            if !ast::is_function_like(self.store_for_node(decl), Some(decl)) {
                continue;
            }
            if i > 0 && self.node_body(decl).is_some() {
                let previous = self
                    .with_symbol_handle_declarations(symbol, |declarations| declarations[i - 1]);
                let decl_parent = self.store_for_node(decl).parent(decl);
                let previous_parent = self.store_for_node(previous).parent(previous);
                let same_parent = match (decl_parent, previous_parent) {
                    (Some(left), Some(right)) => left == right,
                    (None, None) => true,
                    _ => false,
                };
                if same_parent
                    && self.store_for_node(decl).kind(decl)
                        == self.store_for_node(previous).kind(previous)
                    && (self.store_for_node(decl).loc(decl).pos()
                        == self.store_for_node(previous).loc(previous).end()
                        || self
                            .store_for_node(previous)
                            .flags(previous)
                            .intersects(ast::NodeFlags::REPARSED))
                {
                    continue;
                }
            }
            result.push(self.get_signature_from_declaration(decl));
        }
        result
    }

    pub fn get_jsx_factory_entity_for_emit(&mut self, location: ast::Node) -> Option<ast::Node> {
        self.get_jsx_factory_entity(Some(location.clone()))
    }

    pub fn get_jsx_factory_entity_text_for_emit(&mut self, location: ast::Node) -> Option<String> {
        self.get_jsx_namespace(Some(location));
        if let Some(file) = self.try_source_file_for_node(location) {
            let jsx_pragma = ast::get_pragma_from_source_file(Some(file), "jsx");
            if let Some(jsx_pragma) = jsx_pragma {
                let factory = &jsx_pragma.args["factory"].value;
                if parser::parse_isolated_entity_name(factory).is_some() {
                    return Some(factory.clone());
                }
            }
        }
        if self.compiler_options.jsx_factory != ""
            && parser::parse_isolated_entity_name(&self.compiler_options.jsx_factory).is_some()
        {
            return Some(self.compiler_options.jsx_factory.clone());
        }
        Some(format!("{}.createElement", self.jsx_namespace()))
    }

    pub fn get_jsx_fragment_factory_entity_for_emit(
        &mut self,
        location: ast::Node,
    ) -> Option<ast::Node> {
        self.get_jsx_fragment_factory_entity(Some(location.clone()))
    }

    pub fn get_jsx_fragment_factory_entity_text_for_emit(
        &mut self,
        location: ast::Node,
    ) -> Option<String> {
        if let Some(file) = self.try_source_file_for_node(location) {
            let jsx_frag_pragma = ast::get_pragma_from_source_file(Some(file), "jsxfrag");
            if let Some(jsx_frag_pragma) = jsx_frag_pragma {
                let factory = &jsx_frag_pragma.args["factory"].value;
                if parser::parse_isolated_entity_name(factory).is_some() {
                    return Some(factory.clone());
                }
            }
        }
        if self.compiler_options.jsx_fragment_factory != ""
            && parser::parse_isolated_entity_name(&self.compiler_options.jsx_fragment_factory)
                .is_some()
        {
            return Some(self.compiler_options.jsx_fragment_factory.clone());
        }
        None
    }

    pub fn is_optional_parameter_public(&mut self, node: ast::Node) -> bool {
        self.is_optional_parameter(node.clone())
    }

    pub fn is_late_bound(&mut self, node: Option<ast::Node>) -> bool {
        // TODO: Require an emitContext to construct an EmitResolver, remove all emitContext arguments
        // node = r.emitContext.ParseNode(node)
        let Some(node) = node else {
            return false;
        };
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return false;
        }
        let node = node.clone();
        let symbol = self.get_symbol_of_declaration(node);
        let Some(symbol) = symbol else {
            return false;
        };
        self.symbol_handle_check_flags(symbol) & ast::CHECK_FLAGS_LATE != 0
    }

    pub fn get_enum_member_value_for_emit(&mut self, node: ast::Node) -> evaluator::Result {
        // node = r.emitContext.ParseNode(node)
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return evaluator::new_result(evaluator::Value::None, false, false, false);
        }
        let store = self.store_for_node(node);
        self.compute_enum_member_values(store.parent(node).unwrap());
        if !self.semantic_state.has_enum_member_link(node) {
            return evaluator::new_result(evaluator::Value::None, false, false, false);
        }
        self.semantic_state.enum_member_value(node)
    }

    pub fn is_declaration_visible_public(&mut self, node: ast::Node) -> bool {
        // Only lock on external API func to prevent deadlocks
        self.is_declaration_visible(node)
    }

    fn is_declaration_visible(&mut self, node: ast::Node) -> bool {
        // node = r.emitContext.ParseNode(node)
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return false;
        }

        if self.semantic_state.declaration_is_visible(node) == core::TSUnknown {
            // PORT NOTE: reshaped for borrowck.
            let is_visible = self.determine_if_declaration_is_visible(node);
            self.semantic_state.set_declaration_is_visible(
                node,
                if is_visible {
                    core::TSTrue
                } else {
                    core::TSFalse
                },
            );
        }
        self.semantic_state.declaration_is_visible(node) == core::TSTrue
    }

    fn determine_if_declaration_is_visible(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::BindingElement => {
                let parent = store.parent(node).unwrap();
                let grandparent = store.parent(parent).unwrap();
                self.is_declaration_visible(grandparent)
            }
            ast::Kind::VariableDeclaration
            | ast::Kind::ModuleDeclaration
            | ast::Kind::ClassDeclaration
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::FunctionDeclaration
            | ast::Kind::EnumDeclaration
            | ast::Kind::ImportEqualsDeclaration => {
                if ast::is_variable_declaration(store, node) {
                    let name = store.name(node).unwrap();
                    if ast::is_binding_pattern(store, name)
                        && store
                            .elements(name)
                            .is_some_and(|elements| elements.is_empty())
                    {
                        // If the binding pattern is empty, this variable declaration is not visible
                        return false;
                    }
                    // falls through
                }
                // External module augmentation is always visible
                // A @typedef at top-level in an external module is always visible
                if ast::is_external_module_augmentation(store, node)
                    || ast::is_implicitly_exported_js_type_alias(store, node)
                {
                    return true;
                }
                let parent = ast::get_declaration_container(store, node).unwrap();
                // If the node is not exported or it is not ambient module element (except import declaration)
                if self.get_combined_modifier_flags_cached(node.clone())
                    & ast::ModifierFlags::Export
                    == 0
                    && !(store.kind(node) != ast::Kind::ImportEqualsDeclaration
                        && store.kind(parent) != ast::Kind::SourceFile
                        && store.flags(parent) & ast::NodeFlags::Ambient != 0)
                {
                    return ast::is_source_file(store, parent)
                        && !self.source_file_is_external_or_common_js_module(
                            self.source_file_for_node(parent),
                        );
                }
                // Exported members/ambient module elements (exception import declaration) are visible if parent is visible
                self.is_declaration_visible(parent)
            }
            ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature => {
                if self.get_effective_declaration_flags_for_emit(
                    node,
                    ast::ModifierFlags::Private | ast::ModifierFlags::Protected,
                ) != 0
                {
                    // Private/protected properties/methods are not visible
                    return false;
                }
                // Public properties/methods are visible if its parents are visible, so:
                let parent = store.parent(node).unwrap();
                self.is_declaration_visible(parent)
            }
            ast::Kind::Constructor
            | ast::Kind::ConstructSignature
            | ast::Kind::CallSignature
            | ast::Kind::IndexSignature
            | ast::Kind::Parameter
            | ast::Kind::ModuleBlock
            | ast::Kind::FunctionType
            | ast::Kind::ConstructorType
            | ast::Kind::TypeLiteral
            | ast::Kind::TypeReference
            | ast::Kind::ArrayType
            | ast::Kind::TupleType
            | ast::Kind::UnionType
            | ast::Kind::IntersectionType
            | ast::Kind::ParenthesizedType
            | ast::Kind::NamedTupleMember => {
                let parent = store.parent(node).unwrap();
                self.is_declaration_visible(parent)
            }

            // Default binding, import specifier and namespace import is visible
            // only on demand so by default it is not visible
            ast::Kind::ImportClause | ast::Kind::NamespaceImport | ast::Kind::ImportSpecifier => {
                false
            }

            // Type parameters are always visible
            ast::Kind::TypeParameter => true,
            // Source file and namespace export are always visible
            ast::Kind::SourceFile | ast::Kind::NamespaceExportDeclaration => true,

            // Export assignments do not create name bindings outside the module
            ast::Kind::ExportAssignment => false,

            _ => false,
        }
    }

    pub fn precalculate_declaration_emit_visibility(&mut self, file: &ast::SourceFile) {
        if self.semantic_state.declaration_file_aliases_marked(file) {
            return;
        }
        self.semantic_state
            .set_declaration_file_aliases_marked(file, true);
        // TODO: Does this even *have* to be an upfront walk? If it's not possible for a
        // import a = a.b.c statement to chain into exposing a statement in a sibling scope,
        // it could at least be pushed into scope entry -  then it wouldn't need to be recursive.
        // PORT NOTE: reshaped for Rust callback receiver binding.
        let root = file.as_node();
        let _ = file.store().for_each_present_child(root, &mut |child| {
            if self.alias_marking_visitor_worker(child) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
    }
}

pub fn is_common_js_module_exports(
    checker: &Checker<'_, '_>,
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    if !ast::is_binary_expression(store, node) {
        return false;
    }
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if !ast::is_expression_statement(store, parent) {
        return false;
    }
    let Some(parent_parent) = store.parent(parent) else {
        return false;
    };
    if ast::is_source_file(store, parent_parent)
        && checker
            .source_file_binding_state(checker.source_file_for_node(parent_parent))
            .common_js_module_indicator()
            .is_some()
    {
        match ast::get_assignment_declaration_kind(store, node) {
            ast::JSDeclarationKind::ModuleExports | ast::JSDeclarationKind::ExportsProperty => {
                return true;
            }
            _ => {}
        }
    }
    false
}

impl<'a, 'state> Checker<'a, 'state> {
    fn alias_marking_visitor_worker(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::BinaryExpression => {
                let right = store.right(node).unwrap();
                if is_common_js_module_exports(self, store, node)
                    && ast::is_identifier(store, right)
                {
                    self.mark_linked_aliases(right);
                }
            }
            ast::Kind::ExportAssignment => {
                let expression = store.expression(node).unwrap();
                if store.kind(expression) == ast::Kind::Identifier {
                    self.mark_linked_aliases(expression);
                }
            }
            ast::Kind::ExportSpecifier => {
                let property_name_or_name = store.property_name_or_name(node).unwrap();
                self.mark_linked_aliases(property_name_or_name);
            }
            _ => {}
        }
        // PORT NOTE: reshaped for Rust callback receiver binding.
        matches!(
            store.for_each_present_child(node, &mut |child| {
                if self.alias_marking_visitor_worker(child) {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }),
            ControlFlow::Break(())
        )
    }

    // Sets the isVisible link on statements the Identifier or ExportName node points at
    // Follows chains of import d = a.b.c
    fn mark_linked_aliases(&mut self, node: ast::Node) {
        let store = self.store_for_node(node);
        let parent = store.parent(node);
        let mut export_symbol: Option<SymbolIdentity> = None;
        if store.kind(node) != ast::Kind::StringLiteral
            && parent.is_some()
            && (ast::is_export_assignment(store, parent.unwrap())
                || is_common_js_module_exports(self, store, parent.unwrap()))
        {
            let text = store.text(node).to_string();
            export_symbol = self
                .resolve_name(
                    Some(node.clone()),
                    &text,
                    ast::SYMBOL_FLAGS_VALUE
                        | ast::SYMBOL_FLAGS_TYPE
                        | ast::SYMBOL_FLAGS_NAMESPACE
                        | ast::SYMBOL_FLAGS_ALIAS,
                    None, /*nameNotFoundMessage*/
                    false,
                    false,
                )
                .map(SymbolIdentity::from_symbol_handle);
        } else if store.kind(parent.unwrap()) == ast::Kind::ExportSpecifier {
            export_symbol = self.get_target_of_export_specifier(
                parent.unwrap(),
                ast::SYMBOL_FLAGS_VALUE
                    | ast::SYMBOL_FLAGS_TYPE
                    | ast::SYMBOL_FLAGS_NAMESPACE
                    | ast::SYMBOL_FLAGS_ALIAS,
                false,
            );
        }

        let mut visited: HashSet<SymbolIdentity> = HashSet::with_capacity(2); // guard against circular imports
        while let Some(symbol) = export_symbol {
            if visited.contains(&symbol) {
                break;
            }
            visited.insert(symbol);

            let mut next_symbol: Option<SymbolIdentity> = None;
            self.for_each_symbol_handle_declaration(
                symbol.symbol_handle(),
                |checker, declaration| {
                    checker
                        .semantic_state
                        .set_declaration_is_visible(declaration, core::TSTrue);

                    let declaration_store = checker.store_for_node(declaration);
                    if ast::is_internal_module_import_equals_declaration(
                        declaration_store,
                        &declaration,
                    ) {
                        // Add the referenced top container visible
                        let internal_module_reference =
                            declaration_store.module_reference(declaration).unwrap();
                        let first_identifier = ast::get_first_identifier(
                            declaration_store,
                            &internal_module_reference,
                        )
                        .unwrap();
                        let first_identifier_text =
                            declaration_store.text(first_identifier).to_string();
                        let import_symbol = checker
                            .resolve_name(
                                Some(declaration),
                                &first_identifier_text,
                                ast::SYMBOL_FLAGS_VALUE
                                    | ast::SYMBOL_FLAGS_TYPE
                                    | ast::SYMBOL_FLAGS_NAMESPACE
                                    | ast::SYMBOL_FLAGS_ALIAS,
                                None, /*nameNotFoundMessage*/
                                false,
                                false,
                            )
                            .map(SymbolIdentity::from_symbol_handle);
                        next_symbol = import_symbol;
                    }
                },
            );

            export_symbol = next_symbol;
        }
    }
}

pub fn get_meaning_of_entity_name_reference(
    store: &ast::AstStore,
    entity_name: ast::Node,
) -> ast::SymbolFlags {
    // get symbol of the first identifier of the entityName
    let parent = store.parent(entity_name).unwrap();
    if store.kind(parent) == ast::Kind::TypeQuery
        || store.kind(parent) == ast::Kind::ExpressionWithTypeArguments
            && !ast::is_part_of_type_node(store, &parent)
        || store.kind(parent) == ast::Kind::ComputedPropertyName
        || store.kind(parent) == ast::Kind::TypePredicate
            && store.parameter_name(parent).as_ref() == Some(&entity_name)
    {
        // Typeof value
        return ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_EXPORT_VALUE;
    }
    if store.kind(entity_name) == ast::Kind::QualifiedName
        || store.kind(entity_name) == ast::Kind::PropertyAccessExpression
        || store.kind(parent) == ast::Kind::ImportEqualsDeclaration
        || (store.kind(parent) == ast::Kind::QualifiedName
            && store.left(parent).as_ref() == Some(&entity_name))
        || (store.kind(parent) == ast::Kind::PropertyAccessExpression
            && store.expression(parent).as_ref() == Some(&entity_name))
        || (store.kind(parent) == ast::Kind::ElementAccessExpression
            && store.expression(parent).as_ref() == Some(&entity_name))
    {
        // Left identifier from type reference or TypeAlias
        // Entity name of the import declaration
        return ast::SYMBOL_FLAGS_NAMESPACE;
    }
    // Type Reference or TypeAlias entity = Identifier
    ast::SYMBOL_FLAGS_TYPE
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn is_entity_name_visible(
        &mut self,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> printer::SymbolAccessibilityResult {
        self.is_entity_name_visible_worker(entity_name, enclosing_declaration, true)
    }

    fn is_entity_name_visible_worker(
        &mut self,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
        should_compute_alias_to_make_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        // node = r.emitContext.ParseNode(entityName)
        if !ast::is_parse_tree_node(self.store_for_node(entity_name), entity_name) {
            return printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::NotAccessible,
                ..Default::default()
            };
        }

        let entity_store = self.store_for_node(entity_name);
        let meaning = get_meaning_of_entity_name_reference(entity_store, entity_name);
        let first_identifier = ast::get_first_identifier(entity_store, entity_name).unwrap();
        let first_identifier_text = entity_store.text(first_identifier).to_string();

        let symbol = self.resolve_name(
            Some(enclosing_declaration.clone()),
            &first_identifier_text,
            meaning,
            None,
            false,
            false,
        );

        if let Some(symbol) = symbol {
            if self.symbol_handle_flags(symbol) & ast::SYMBOL_FLAGS_TYPE_PARAMETER != 0
                && meaning & ast::SYMBOL_FLAGS_TYPE != 0
            {
                return printer::SymbolAccessibilityResult {
                    accessibility: printer::SymbolAccessibility::Accessible,
                    ..Default::default()
                };
            }
        }

        if symbol.is_none() && ast::is_this_identifier(entity_store, first_identifier) {
            let first_identifier = first_identifier;
            let container = self.get_this_container(first_identifier, false, false);
            let sym = self.get_symbol_of_declaration(container);
            if let Some(sym) = sym {
                if self
                    .is_symbol_accessible_by_identity(
                        Some(SymbolIdentity::from_symbol_handle(sym)),
                        Some(enclosing_declaration.clone()),
                        meaning,
                        false,
                    )
                    .accessibility
                    == printer::SymbolAccessibility::Accessible
                {
                    return printer::SymbolAccessibilityResult {
                        accessibility: printer::SymbolAccessibility::Accessible,
                        ..Default::default()
                    };
                }
            }
        }

        let Some(symbol) = symbol else {
            return printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::NotResolved,
                error_symbol_name: first_identifier_text.clone(),
                error_node: Some(first_identifier),
                ..Default::default()
            };
        };

        let flags = self.symbol_handle_flags(symbol);
        let visible = self.has_visible_declarations_worker(
            symbol,
            flags,
            should_compute_alias_to_make_visible,
        );
        if let Some(visible) = visible {
            return visible;
        }

        printer::SymbolAccessibilityResult {
            accessibility: printer::SymbolAccessibility::NotAccessible,
            error_symbol_name: first_identifier_text,
            error_node: Some(first_identifier),
            ..Default::default()
        }
    }
}

pub fn noop_add_visible_alias(_declaration: ast::Node, _aliasing_statement: ast::Node) {}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn has_visible_declarations_by_identity(
        &mut self,
        symbol: SymbolIdentity,
        should_compute_alias_to_make_visible: bool,
    ) -> Option<printer::SymbolAccessibilityResult> {
        let symbol = symbol.symbol_handle();
        let flags = self.symbol_handle_flags(symbol);
        self.has_visible_declarations_worker(symbol, flags, should_compute_alias_to_make_visible)
    }

    fn has_visible_declarations_worker(
        &mut self,
        symbol: ast::SymbolHandle,
        symbol_flags: ast::SymbolFlags,
        should_compute_alias_to_make_visible: bool,
    ) -> Option<printer::SymbolAccessibilityResult> {
        let mut aliases_to_make_visible_set: Option<HashMap<ast::NodeId, ast::Node>> = None;

        let inaccessible = self.any_symbol_handle_declaration(symbol, |resolver, declaration| {
            if {
                let declaration_store = resolver.store_for_node(declaration);
                ast::is_identifier(declaration_store, declaration)
            } {
                return false;
            }

            let declaration_visible = resolver.is_declaration_visible(declaration);
            if !declaration_visible {
                let mut add_visible_alias =
                    |resolver: &mut Checker<'a, '_>, aliasing_statement: ast::Node| {
                        if should_compute_alias_to_make_visible {
                            resolver
                                .semantic_state
                                .set_declaration_is_visible(declaration, core::TSTrue);
                            if aliases_to_make_visible_set.is_none() {
                                aliases_to_make_visible_set = Some(HashMap::new());
                            }
                            aliases_to_make_visible_set.as_mut().unwrap().insert(
                                ast::get_node_id(
                                    resolver.store_for_node(declaration),
                                    declaration,
                                ),
                                aliasing_statement.clone(),
                            );
                        } else {
                            noop_add_visible_alias(declaration, aliasing_statement);
                        }
                    };

                // Mark the unexported alias as visible if its parent is visible
                // because these kind of aliases can be used to name types in declaration file
                let any_import_syntax = {
                    let declaration_store = resolver.store_for_node(declaration);
                    get_any_import_syntax(declaration_store, declaration)
                };
                if let Some(any_import_syntax) = any_import_syntax {
                    let any_import_syntax_parent = resolver
                        .store_for_node(any_import_syntax)
                        .parent(any_import_syntax)
                        .unwrap();
                    if !ast::has_syntactic_modifier(resolver.store_for_node(any_import_syntax), any_import_syntax, ast::ModifierFlags::Export) && // import clause without export
                        resolver.is_declaration_visible(any_import_syntax_parent)
                    {
                        add_visible_alias(resolver, any_import_syntax);
                        return false;
                    }
                }
                let (declaration_parent, declaration_grandparent, declaration_great_grandparent) = {
                    let declaration_store = resolver.store_for_node(declaration);
                    let declaration_parent = declaration_store.parent(declaration);
                    let declaration_grandparent =
                        declaration_parent.and_then(|parent| declaration_store.parent(parent));
                    let declaration_great_grandparent =
                        declaration_grandparent.and_then(|parent| declaration_store.parent(parent));
                    (
                        declaration_parent,
                        declaration_grandparent,
                        declaration_great_grandparent,
                    )
                };
                if ast::is_variable_declaration(resolver.store_for_node(declaration), declaration)
                    && declaration_grandparent.as_ref().is_some_and(|grandparent| {
                        ast::is_variable_statement(
                            resolver.store_for_node(*grandparent),
                            *grandparent,
                        )
                    })
                    && declaration_grandparent.as_ref().is_some_and(|grandparent| {
                        !ast::has_syntactic_modifier(
                            resolver.store_for_node(*grandparent),
                            *grandparent,
                            ast::ModifierFlags::Export,
                        )
                    })
                    && declaration_great_grandparent
                        .as_ref()
                        .is_some_and(|great_grandparent| {
                            resolver.is_declaration_visible(*great_grandparent)
                        })
                {
                    add_visible_alias(resolver, declaration_grandparent.unwrap());
                    return false;
                }
                if ast::is_late_visibility_painted_statement(resolver.store_for_node(declaration), &declaration) && // unexported top-level statement
                    !ast::has_syntactic_modifier(resolver.store_for_node(declaration), declaration, ast::ModifierFlags::Export) &&
                    declaration_parent.as_ref().is_some_and(|parent| resolver.is_declaration_visible(*parent))
                {
                    add_visible_alias(resolver, declaration);
                    return false;
                }
                if ast::is_binding_element(resolver.store_for_node(declaration), declaration) {
                    let binding_parent = declaration_parent;
                    let (
                        binding_grandparent,
                        _binding_great_grandparent,
                        binding_statement,
                        binding_statement_parent,
                    ) = {
                        let declaration_store = resolver.store_for_node(declaration);
                        let binding_grandparent =
                            binding_parent.and_then(|parent| declaration_store.parent(parent));
                        let binding_great_grandparent =
                            binding_grandparent.and_then(|parent| declaration_store.parent(parent));
                        let binding_statement = binding_great_grandparent
                            .and_then(|parent| declaration_store.parent(parent));
                        let binding_statement_parent =
                            binding_statement.and_then(|parent| declaration_store.parent(parent));
                        (
                            binding_grandparent,
                            binding_great_grandparent,
                            binding_statement,
                            binding_statement_parent,
                        )
                    };
                    if symbol_flags & ast::SYMBOL_FLAGS_ALIAS != 0
                        && ast::is_in_js_file(resolver.store_for_node(declaration), declaration)
                        && binding_parent.is_some()
                        && binding_grandparent.is_some()
                        && binding_grandparent.as_ref().is_some_and(|grandparent| {
                            ast::is_variable_declaration(
                                resolver.store_for_node(*grandparent),
                                *grandparent,
                            )
                        })
                        && binding_statement.is_some()
                        && binding_statement.as_ref().is_some_and(|statement| {
                            ast::is_variable_statement(
                                resolver.store_for_node(*statement),
                                *statement,
                            )
                        })
                        && binding_statement.as_ref().is_some_and(|statement| {
                            !ast::has_syntactic_modifier(
                                resolver.store_for_node(*statement),
                                *statement,
                                ast::ModifierFlags::Export,
                            )
                        })
                        && binding_statement_parent.is_some()
                        && binding_statement_parent
                            .as_ref()
                            .is_some_and(|parent| resolver.is_declaration_visible(*parent))
                    {
                        add_visible_alias(resolver, binding_statement.unwrap());
                        return false;
                    }
                    if symbol_flags & ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE != 0 {
                        let root_declaration = ast::walk_up_binding_elements_and_patterns(
                            resolver.store_for_node(declaration),
                            &declaration,
                        )
                        .unwrap();
                        if ast::is_parameter_declaration(
                            resolver.store_for_node(root_declaration),
                            root_declaration,
                        ) {
                            return true;
                        }
                        let variable_statement = {
                            let declaration_store = resolver.store_for_node(root_declaration);
                            declaration_store
                                .parent(root_declaration)
                                .and_then(|parent| declaration_store.parent(parent))
                                .unwrap()
                        };
                        if !ast::is_variable_statement(
                            resolver.store_for_node(variable_statement),
                            variable_statement,
                        ) {
                            return true;
                        }
                        if ast::has_syntactic_modifier(
                            resolver.store_for_node(variable_statement),
                            variable_statement,
                            ast::ModifierFlags::Export,
                        ) {
                            return false; // no alias to add, already exported
                        }
                        let variable_statement_parent = resolver
                            .store_for_node(variable_statement)
                            .parent(variable_statement)
                            .unwrap();
                        if !resolver.is_declaration_visible(variable_statement_parent) {
                            return true; // not visible
                        }
                        add_visible_alias(resolver, variable_statement);
                        return false;
                    }
                }

                // Declaration is not visible
                return true;
            }

            false
        });

        if inaccessible {
            return None;
        }

        Some(printer::SymbolAccessibilityResult {
            accessibility: printer::SymbolAccessibility::Accessible,
            aliases_to_make_visible: aliases_to_make_visible_set
                .map(|m| m.into_values().collect())
                .unwrap_or_default(),
            ..Default::default()
        })
    }

    pub fn is_implementation_of_overload(&mut self, node: ast::Node) -> bool {
        // node = r.emitContext.ParseNode(node)
        let node_as_node = node;
        let node_store = self.store_for_node(node_as_node);
        if !ast::is_parse_tree_node(node_store, node_as_node) {
            return false;
        }
        let body = node_store.body(node_as_node);
        if ast::node_is_present(node_store, body) {
            if ast::is_get_accessor_declaration(node_store, node_as_node)
                || ast::is_set_accessor_declaration(node_store, node_as_node)
            {
                return false; // Get or set accessors can never be overload implementations, but can have up to 2 signatures
            }
            let symbol = self.get_symbol_of_declaration(node_as_node);
            let signatures_of_symbol = self.get_signatures_of_symbol_handle(symbol);
            // If this function body corresponds to function with multiple signature, it is implementation of overload
            // e.g.: function foo(a: string): string;
            //       function foo(a: number): number;
            //       function foo(a: any) { // This is implementation of the overloads
            //           return a;
            //       }
            if signatures_of_symbol.len() > 1 {
                return true;
            }
            // If there is single signature for the symbol, it is overload if that signature isn't coming from the node
            // e.g.: function foo(a: string): string;
            //       function foo(a: any) { // This is implementation of the overloads
            //           return a;
            //       }
            if signatures_of_symbol.len() == 1 {
                let declaration = self.signature_record(signatures_of_symbol[0]).declaration;
                if declaration != Some(node_as_node) && declaration.is_some() {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_first_declaration_of_symbol(&mut self, node: ast::Node) -> bool {
        let Some(symbol) = self.get_symbol_of_declaration(node) else {
            return true;
        };
        self.with_symbol_handle_declarations(symbol, |declarations| {
            declarations.first().is_none_or(|first| *first == node)
        })
    }

    pub fn is_import_required_by_augmentation(&mut self, decl: ast::Node) -> bool {
        // node = r.emitContext.ParseNode(node)
        let decl_store = self.store_for_node(decl);
        let Some(module_specifier) = decl_store.module_specifier(decl) else {
            return false;
        };
        let Some(decl_node) = decl_store.parent(module_specifier) else {
            return false;
        };
        let decl_node = decl_node;
        if !ast::is_parse_tree_node(decl_store, decl_node) {
            return false;
        }
        let file = ast::get_source_file_of_node(decl_store, Some(decl_node));
        let Some(file) = file else {
            return false;
        };
        let file = self.source_file_for_node(file);
        let Some(file_symbol) = self.source_file_symbol(file) else {
            // script file
            return false;
        };
        let import_target = self.get_external_module_file_from_declaration_for_emit(decl_node);
        let Some(import_target) = import_target else {
            return false;
        };
        if import_target.path() == file.path() {
            return false;
        }
        let exports = self
            .collect_exports_of_module_identity(SymbolIdentity::from_symbol_handle(file_symbol));
        for symbol in exports.into_values() {
            let merged = self.get_merged_symbol_identity(Some(symbol)).unwrap();
            if self
                .collect_symbol_identity_declarations(merged)
                .into_iter()
                .any(|d| {
                    let store = self.store_for_node(d);
                    let decl_file = ast::get_source_file_of_node(store, Some(d));
                    decl_file.is_some_and(|decl_file| {
                        self.source_file_for_node(decl_file).path() == import_target.path()
                    })
                })
            {
                return true;
            }
        }
        false
    }

    pub fn is_definitely_reference_to_global_symbol_object(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let name = store.name(node);
        let expression = store.expression(node);
        let Some(name) = name else {
            return false;
        };
        let Some(expression) = expression else {
            return false;
        };
        if !ast::is_property_access_expression(store, node)
            || !ast::is_identifier(store, name)
            || !ast::is_property_access_expression(store, expression)
                && !ast::is_identifier(store, expression)
        {
            return false;
        }
        if store.kind(expression) == ast::Kind::Identifier {
            if store.text(expression) != "Symbol" {
                return false;
            }
            let expression = expression;
            // Exactly `Symbol.something` and `Symbol` either does not resolve or definitely resolves to the global Symbol
            let resolved = self.get_resolved_symbol(expression);
            let global_symbol = self.get_global_symbol(
                "Symbol",
                ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_EXPORT_VALUE,
                None, /*diagnostic*/
            );
            return self.same_optional_symbol_identity(Some(resolved), global_symbol);
        }
        let lhs = store.expression(expression);
        let lhs_name = store.name(expression);
        let Some(lhs) = lhs else {
            return false;
        };
        let Some(lhs_name) = lhs_name else {
            return false;
        };
        if store.kind(lhs) != ast::Kind::Identifier
            || store.text(lhs) != "globalThis"
            || store.text(lhs_name) != "Symbol"
        {
            return false;
        }
        let lhs = lhs;
        // Exactly `globalThis.Symbol.something` and `globalThis` resolves to the global `globalThis`
        let resolved = self.get_resolved_symbol(lhs);
        let global_this = self.global_this_symbol_identity();
        self.same_symbol_identity(resolved, global_this)
    }

    pub fn requires_adding_implicit_undefined(
        &mut self,
        declaration: ast::Node,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        if !ast::is_parse_tree_node(self.store_for_node(declaration), declaration) {
            return false;
        }
        self.requires_adding_implicit_undefined_worker_public(
            declaration,
            symbol,
            enclosing_declaration,
        )
    }

    pub fn requires_adding_implicit_undefined_unsafe(
        &mut self,
        declaration: ast::Node,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        if !ast::is_parse_tree_node(self.store_for_node(declaration), declaration) {
            return false;
        }
        // NO LOCKING - only should be called in contexts that already have a checker lock
        self.requires_adding_implicit_undefined_worker_public(
            declaration,
            symbol,
            enclosing_declaration,
        )
    }

    fn requires_adding_implicit_undefined_worker_public(
        &mut self,
        declaration: ast::Node,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        // node = r.emitContext.ParseNode(node)
        let declaration_store = self.store_for_node(declaration);
        if !ast::is_parse_tree_node(declaration_store, declaration) {
            return false;
        }
        match declaration_store.kind(declaration) {
            ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => {
                if let Some(symbol) = symbol {
                    let t = self.get_type_of_symbol_identity(symbol);
                    let symbol_flags = self.symbol_identity_flags(symbol);
                    let has_mapped_type =
                        self.semantic_state.has_reverse_mapped_symbol_links(symbol)
                            && self
                                .semantic_state
                                .reverse_mapped_mapped_type(symbol)
                                .is_some();
                    return (symbol_flags & ast::SYMBOL_FLAGS_PROPERTY != 0)
                        && (symbol_flags & ast::SYMBOL_FLAGS_OPTIONAL != 0)
                        && is_optional_declaration(self.store_for_node(declaration), declaration)
                        && has_mapped_type
                        && contains_non_missing_undefined_type(self, t);
                }
                let symbol = self.get_symbol_of_declaration(declaration.clone()).unwrap();
                let symbol_identity = SymbolIdentity::from_symbol_handle(symbol);
                let symbol_flags = self.symbol_handle_flags(symbol);
                let t = self.get_type_of_symbol_handle(symbol);
                let has_mapped_type = self
                    .semantic_state
                    .has_reverse_mapped_symbol_links(symbol_identity)
                    && self
                        .semantic_state
                        .reverse_mapped_mapped_type(symbol_identity)
                        .is_some();
                (symbol_flags & ast::SYMBOL_FLAGS_PROPERTY != 0)
                    && (symbol_flags & ast::SYMBOL_FLAGS_OPTIONAL != 0)
                    && is_optional_declaration(self.store_for_node(declaration), declaration)
                    && has_mapped_type
                    && contains_non_missing_undefined_type(self, t)
            }
            ast::Kind::Parameter => {
                self.requires_adding_implicit_undefined_worker(declaration, enclosing_declaration)
            }
            _ => panic!("Node cannot possibly require adding undefined"),
        }
    }

    fn requires_adding_implicit_undefined_worker(
        &mut self,
        parameter: ast::Node,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        (self.is_required_initialized_parameter(parameter, enclosing_declaration)
            || self.is_optional_uninitialized_parameter_property(parameter))
            && !self.declared_parameter_type_contains_undefined(parameter)
    }

    fn declared_parameter_type_contains_undefined(&mut self, parameter: ast::Node) -> bool {
        let type_node = self.store_for_node(parameter).type_node(parameter);
        let Some(type_node) = type_node else {
            return false;
        };
        let type_node = type_node;
        let t = self.get_type_from_type_node(type_node);
        // allow error type here to avoid confusing errors that the annotation has to contain undefined when it does in cases like this:
        //
        // export function fn(x?: Unresolved | undefined): void {}
        self.is_error_type(t) || self.contains_undefined_type(t)
    }

    fn is_optional_uninitialized_parameter_property(&mut self, parameter: ast::Node) -> bool {
        self.strict_null_checks()
            && self.is_optional_parameter(parameter.clone())
            && self
                .store_for_node(parameter)
                .initializer(parameter)
                .is_none()
            && ast::has_syntactic_modifier(
                self.store_for_node(parameter),
                parameter,
                ast::ModifierFlags::ParameterPropertyModifier,
            )
    }

    fn is_required_initialized_parameter(
        &mut self,
        parameter: ast::Node,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        if !self.strict_null_checks()
            || self.is_optional_parameter(parameter.clone())
            || self
                .store_for_node(parameter)
                .initializer(parameter)
                .is_none()
        {
            return false;
        }
        if ast::has_syntactic_modifier(
            self.store_for_node(parameter),
            parameter,
            ast::ModifierFlags::ParameterPropertyModifier,
        ) {
            return enclosing_declaration.is_some_and(|enclosing_declaration| {
                ast::is_function_like_declaration(
                    self.store_for_node(enclosing_declaration),
                    Some(enclosing_declaration),
                )
            });
        }
        true
    }

    fn is_optional_parameter_for_emit(&mut self, node: ast::Node) -> bool {
        self.is_optional_parameter(node.clone())
    }

    pub fn is_literal_const_declaration(&mut self, node: ast::Node) -> bool {
        // node = r.emitContext.ParseNode(node)
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return false;
        }
        let store = self.store_for_node(node);
        if is_declaration_readonly(store, node)
            || ast::is_variable_declaration(store, node) && ast::is_var_const(store, node)
        {
            let symbol = self.get_symbol_of_declaration(node.clone()).unwrap();
            let type_of_symbol = self.get_type_of_symbol_handle(symbol);
            return is_fresh_literal_type(self, type_of_symbol);
        }
        false
    }

    pub fn is_expando_function_declaration_unsafe(&mut self, node: ast::Node) -> bool {
        // node = r.emitContext.ParseNode(node)
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return false;
        }
        if ast::is_variable_declaration(self.store_for_node(node), node) {
            let store = self.store_for_node(node);
            if store.r#type(node).is_some()
                || (!ast::is_in_js_file(store, node) && !self.is_var_const_like(node))
            {
                return false;
            }
            let Some(initializer) = self.get_declared_expando_initializer(node) else {
                return false;
            };
            if self
                .store_for_node(initializer)
                .r#type(initializer)
                .is_some()
            {
                return false;
            }
        }
        self.get_properties_of_container_function(Some(node))
            .into_iter()
            .any(|property| {
                self.symbol_value_declaration(property.into())
                    .is_some_and(|declaration| {
                        ast::is_expando_property_declaration(
                            self.store_for_node(declaration),
                            Some(declaration),
                        )
                    })
            })
    }

    pub fn is_expando_function_declaration(&mut self, node: ast::Node) -> bool {
        self.is_expando_function_declaration_unsafe(node)
    }

    pub fn set_expando_namespace_metadata(
        &mut self,
        synthesized_namespace: ast::Node,
        declaration: ast::Node,
        properties: &[ast::SymbolIdentity],
    ) {
        if let Some(symbol) = self.get_symbol_of_declaration(declaration) {
            self.semantic_state.set_synthetic_node_symbol_identity(
                synthesized_namespace,
                SymbolIdentity::from_symbol_handle(symbol),
            );
        }
        let locals = if properties.is_empty() {
            SymbolIdentityTable::default()
        } else {
            let symbols = properties
                .iter()
                .map(|symbol| {
                    (
                        self.missing_name_symbol_identity_name(SymbolIdentity::from_symbol_handle(
                            symbol.symbol_handle(),
                        ))
                        .into(),
                        SymbolIdentity::from_symbol_handle(symbol.symbol_handle()),
                    )
                })
                .collect::<Vec<_>>();
            crate::utilities::create_symbol_table(&symbols)
        };
        self.semantic_state
            .set_synthetic_node_locals(synthesized_namespace, locals);
    }

    pub fn should_emit_function_properties(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if store.body(node).is_some() {
            return true;
        }
        let Some(symbol) = self.get_symbol_of_declaration(node) else {
            return true;
        };
        !self.with_symbol_handle_declarations(symbol, |declarations| {
            declarations.iter().copied().all(|declaration| {
                let store = self.store_for_node(declaration);
                !ast::is_function_declaration(store, declaration)
                    || store.body(declaration).is_none()
            })
        })
    }

    pub fn is_symbol_accessible_public(
        &mut self,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: ast::Node,
        meaning: ast::SymbolFlags,
        should_compute_alias_to_mark_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        // TODO: Split into locking and non-locking API methods - only current usage is the symbol tracker, which is non-locking,
        // as all tracker calls happen within a CreateX call below, which already holds a lock
        // r.checkerMu.Lock()
        // defer r.checkerMu.Unlock()
        self.is_symbol_accessible_by_identity(
            symbol,
            Some(enclosing_declaration.clone()),
            meaning,
            should_compute_alias_to_mark_visible,
        )
    }
}

pub(crate) fn is_const_enum_or_const_enum_only_module(
    checker: &Checker<'_, '_>,
    s: Option<SymbolIdentity>,
) -> bool {
    let Some(s) = s.map(SymbolIdentity::symbol_handle) else {
        return false;
    };
    let flags = checker.symbol_handle_flags(s);
    flags & ast::SYMBOL_FLAGS_CONST_ENUM != 0
        || flags & ast::SYMBOL_FLAGS_CONST_ENUM_ONLY_MODULE != 0
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn is_referenced_alias_declaration(&mut self, node: ast::Node) -> bool {
        if !self.can_collect_symbol_alias_accessibility_data()
            || !ast::is_parse_tree_node(self.store_for_node(node), node)
        {
            return true;
        }

        let store = self.store_for_node(node);
        if ast::is_alias_symbol_declaration(store, node) {
            if let Some(symbol) = self.get_symbol_of_declaration(node.clone()) {
                let referenced = self.semantic_state.alias_symbol_referenced(symbol);
                let target = self.semantic_state.alias_symbol_target(symbol);
                if referenced {
                    return true;
                }
                if let Some(target) = target {
                    if ast::get_combined_modifier_flags(self.store_for_node(node), node)
                        & ast::ModifierFlags::Export
                        != 0
                        && self.symbol_identity_flags_for_emit(target) & ast::SYMBOL_FLAGS_VALUE
                            != 0
                        && (self.compiler_options.should_preserve_const_enums()
                            || !self
                                .is_const_enum_or_const_enum_only_module_identity_for_emit(target))
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_value_alias_declaration_public(&mut self, node: ast::Node) -> bool {
        if !self.can_collect_symbol_alias_accessibility_data()
            || !ast::is_parse_tree_node(self.store_for_node(node), node)
        {
            return true;
        }

        self.is_value_alias_declaration_worker(node)
    }

    pub fn is_value_alias_declaration(&mut self, node: ast::Node) -> bool {
        self.is_value_alias_declaration_public(node)
    }

    fn is_value_alias_declaration_worker(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::ImportEqualsDeclaration => {
                let symbol = self.get_symbol_of_declaration(node.clone());
                self.is_alias_resolved_to_value(symbol, false /*excludeTypeOnlyValues*/)
            }
            ast::Kind::ImportClause
            | ast::Kind::NamespaceImport
            | ast::Kind::ImportSpecifier
            | ast::Kind::ExportSpecifier => {
                let symbol = self.get_symbol_of_declaration(node.clone());
                symbol.is_some_and(|symbol| {
                    self.is_alias_resolved_to_value(
                        Some(symbol),
                        true, /*excludeTypeOnlyValues*/
                    )
                })
            }
            ast::Kind::ExportDeclaration => {
                let export_clause = store.export_clause(node);
                if let Some(export_clause) = export_clause {
                    if ast::is_namespace_export(store, export_clause) {
                        return true;
                    }
                    for n in store.elements(export_clause).into_iter().flatten() {
                        if self.is_value_alias_declaration_worker(n) {
                            return true;
                        }
                    }
                }
                false
            }
            ast::Kind::ExportAssignment => {
                if self
                    .store_for_node(node)
                    .expression(node)
                    .is_some_and(|expression| store.kind(expression) == ast::Kind::Identifier)
                {
                    let symbol = self.get_symbol_of_declaration(node.clone());
                    if let Some(symbol) = symbol {
                        if !self
                            .symbol_handle_flags(symbol)
                            .intersects(ast::SYMBOL_FLAGS_ALIAS)
                        {
                            return true;
                        }
                        return self.is_alias_resolved_to_value(
                            Some(symbol),
                            true, /*excludeTypeOnlyValues*/
                        );
                    }
                    return false;
                }
                true
            }
            ast::Kind::BinaryExpression => {
                let store = self.store_for_node(node);
                let right = store.right(node).unwrap();
                if is_common_js_module_exports(self, store, node)
                    && ast::is_identifier(store, right)
                {
                    let symbol = self.get_symbol_of_declaration(node.clone());
                    return self
                        .is_alias_resolved_to_value(symbol, true /*excludeTypeOnlyValues*/);
                }
                false
            }
            _ => false,
        }
    }

    fn is_alias_resolved_to_value(
        &mut self,
        symbol: Option<ast::SymbolHandle>,
        exclude_type_only_values: bool,
    ) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        if let Some(value_declaration) = self.symbol_handle_value_declaration(symbol) {
            let store = self.store_for_node(value_declaration);
            if let Some(container) = ast::get_source_file_of_node(store, Some(value_declaration)) {
                let container_node = container;
                let file_symbol = self.get_symbol_of_declaration(container_node);
                if let Some(file_symbol) = file_symbol {
                    // Ensures cjs export assignment is setup, since this symbol may point at, and merge with, the file itself.
                    // If we don't, the merge may not have yet occurred, and the flags check below will be missing flags that
                    // are added as a result of the merge.
                    self.resolve_external_module_symbol_handle(
                        file_symbol,
                        false, /*dontResolveAlias*/
                    );
                }
            }
        }
        let resolved_alias =
            self.resolve_alias_identity(SymbolIdentity::from_symbol_handle(symbol));
        let target = self
            .get_export_symbol_of_value_symbol_identity_if_exported_for_emit(Some(resolved_alias));
        if target
            .as_ref()
            .is_some_and(|target| self.is_unknown_symbol_identity(*target))
        {
            return !exclude_type_only_values
                || self
                    .get_type_only_alias_declaration_handle(symbol)
                    .is_none();
        }
        // const enums and modules that contain only const enums are not considered values from the emit perspective
        // unless 'preserveConstEnums' option is set to true
        let symbol_flags = self.get_symbol_flags_ex(
            SymbolIdentity::from_symbol_handle(symbol),
            exclude_type_only_values,
            true, /*excludeLocalMeanings*/
        );
        symbol_flags & ast::SYMBOL_FLAGS_VALUE != 0
            && (self.compiler_options.should_preserve_const_enums()
                || target.is_none_or(|target| {
                    !self.is_const_enum_or_const_enum_only_module_identity_for_emit(target)
                }))
    }

    pub fn is_top_level_value_import_equals_with_entity_name(&mut self, node: ast::Node) -> bool {
        if !self.can_collect_symbol_alias_accessibility_data() {
            return true;
        }
        let store = self.store_for_node(node);
        if !ast::is_parse_tree_node(store, node)
            || store.kind(node) != ast::Kind::ImportEqualsDeclaration
            || store
                .parent(node)
                .is_none_or(|parent| store.kind(parent) != ast::Kind::SourceFile)
        {
            return false;
        }
        if ast::is_import_equals_declaration(store, node) && {
            let module_reference = store.module_reference(node);
            ast::node_is_missing(store, module_reference)
                || module_reference.is_some_and(|module_reference| {
                    store.kind(module_reference) == ast::Kind::ExternalModuleReference
                })
        } {
            return false;
        }

        let symbol = self.get_symbol_of_declaration(node.clone());
        self.is_alias_resolved_to_value(symbol, false /*excludeTypeOnlyValues*/)
    }

    pub fn mark_linked_references_recursively(&mut self, file: Option<&ast::SourceFile>) {
        if let Some(file) = file {
            if ast::is_source_file_js(file) {
                return;
            }

            let mut stack = Vec::new();
            let _ = file
                .store()
                .for_each_present_child(file.as_node(), |child| {
                    stack.push(child);
                    ControlFlow::Continue(())
                });

            while let Some(n) = stack.pop() {
                let store = self.store_for_node(n);
                if ast::is_import_equals_declaration(store, n)
                    && ast::get_combined_modifier_flags(store, n) & ast::ModifierFlags::Export == 0
                {
                    continue; // These are deferred and marked in a chain when referenced
                }
                if ast::is_import_declaration(store, n) {
                    continue; // likewise, these are ultimately what get marked by calls on other nodes - we want to skip them
                }
                self.mark_linked_references(
                    n,
                    REFERENCE_HINT_UNSPECIFIED,
                    None, /*propSymbol*/
                    None, /*parentType*/
                );

                let store = self.store_for_node(n);
                let mut children = Vec::new();
                let _ = store.for_each_present_child(n, |child| {
                    children.push(child);
                    ControlFlow::Continue(())
                });
                for child in children.into_iter().rev() {
                    stack.push(child);
                }
            }
        }
    }

    pub fn get_external_module_file_from_declaration_for_emit(
        &mut self,
        declaration: ast::Node,
    ) -> Option<ast::SourceFile> {
        if !ast::is_parse_tree_node(self.store_for_node(declaration), declaration) {
            return None;
        }
        self.get_external_module_file_from_declaration(declaration.clone())
            .map(ast::SourceFile::share_readonly)
    }

    fn reference_get_resolved_symbol(&mut self, node: Option<ast::Node>) -> Option<SymbolIdentity> {
        self.node_resolved_symbol(node?)
    }

    fn reference_get_merged_symbol(
        &mut self,
        symbol: Option<SymbolIdentity>,
    ) -> Option<SymbolIdentity> {
        self.get_merged_symbol_identity(symbol)
    }

    fn reference_get_parent_of_symbol(
        &mut self,
        symbol: Option<SymbolIdentity>,
    ) -> Option<SymbolIdentity> {
        self.symbol_identity_parent_for_emit(symbol?)
    }

    fn reference_get_symbol_of_declaration(
        &mut self,
        declaration: Option<ast::Node>,
    ) -> Option<SymbolIdentity> {
        self.get_symbol_of_declaration(declaration?)
            .map(SymbolIdentity::from_symbol_handle)
    }

    fn reference_get_referenced_value_symbol(
        &mut self,
        reference: ast::IdentifierNode,
        start_in_declaration_container: bool,
    ) -> Option<SymbolIdentity> {
        if let Some(resolved_symbol) = self.reference_get_resolved_symbol(Some(reference)) {
            return Some(resolved_symbol);
        }

        let reference_store = self.store_for_node(reference);
        let mut location = reference;
        let reference_parent = reference_store.parent(reference);
        if start_in_declaration_container
            && reference_parent.as_ref().is_some_and(|parent| {
                ast::is_declaration(reference_store, *parent)
                    && reference_store.name(*parent) == Some(reference)
            })
        {
            if let Some(container) =
                ast::get_declaration_container(reference_store, *reference_parent.as_ref().unwrap())
            {
                location = container;
            }
        }

        let reference_text = reference_store.text(reference).to_string();
        self.resolve_name(
            Some(location),
            &reference_text,
            ast::SYMBOL_FLAGS_EXPORT_VALUE | ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_ALIAS,
            None,  /* nameNotFoundMessage */
            false, /* isUse */
            false, /* excludeGlobals */
        )
        .map(SymbolIdentity::from_symbol_handle)
    }

    fn reference_get_export_symbol_of_value_symbol_if_exported(
        &mut self,
        symbol: Option<SymbolIdentity>,
    ) -> Option<SymbolIdentity> {
        self.get_export_symbol_of_value_symbol_identity_if_exported_for_emit(symbol)
    }

    fn reference_get_element_access_expression_name(&mut self, expression: ast::Node) -> String {
        let expression_store = self.store_for_node(expression);
        let Some(argument) = expression_store.argument_expression(expression) else {
            return String::new();
        };
        let argument = argument;
        let argument_store = self.store_for_node(argument);
        let (name, ok) = if ast::is_string_or_numeric_literal_like(argument_store, argument) {
            (argument_store.text(argument).to_string(), true)
        } else if ast::is_entity_name_expression(self.store_for_node(argument), argument) {
            self.reference_try_get_name_from_entity_name_expression(argument)
        } else {
            (String::new(), false)
        };
        if ok { name } else { String::new() }
    }

    fn reference_try_get_name_from_entity_name_expression(
        &mut self,
        node: ast::Node,
    ) -> (String, bool) {
        let symbol = self.resolve_entity_name(
            node,
            ast::SYMBOL_FLAGS_VALUE,
            true, /*ignoreErrors*/
            false,
            None,
        );
        let Some(symbol) = symbol else {
            return (String::new(), false);
        };

        let is_const_like = self.is_constant_variable_identity_for_emit(symbol)
            || self.symbol_identity_flags_for_emit(symbol) & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0;
        if !is_const_like {
            return (String::new(), false);
        }

        let Some(declaration) = self.symbol_identity_value_declaration_for_emit(symbol) else {
            return (String::new(), false);
        };
        let declaration = declaration;

        if let Some(t) = self.try_get_type_from_type_node(declaration) {
            if let Some(name) = self.try_get_name_from_type(t) {
                return (name, true);
            }
        }

        let declaration_store = self.store_for_node(declaration);
        if has_only_expression_initializer(declaration_store, declaration)
            && self.is_block_scoped_name_declared_before_use(declaration, node)
        {
            if let Some(initializer) = declaration_store.initializer(declaration) {
                let declaration_parent = declaration_store.parent(declaration).unwrap();
                let initializer_type =
                    if ast::is_binding_pattern(declaration_store, declaration_parent) {
                        self.get_type_for_binding_element(declaration)
                    } else {
                        self.get_type_of_expression(initializer)
                    };
                if let Some(name) = self.try_get_name_from_type(initializer_type) {
                    return (name, true);
                }
            } else if ast::is_enum_member(declaration_store, declaration) {
                let name = declaration_store.name(declaration).unwrap();
                return ast::try_get_text_of_property_name(declaration_store, name);
            }
        }

        (String::new(), false)
    }

    pub fn get_referenced_export_container(
        &mut self,
        node: ast::IdentifierNode,
        prefix_locals: bool,
    ) -> Option<ast::Node> /*SourceFile|ModuleDeclaration|EnumDeclaration*/ {
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return None;
        }

        let node_store = self.store_for_node(node);
        let start_in_declaration_container = node_store.parent(node).is_some_and(|parent| {
            (node_store.kind(parent) == ast::Kind::ModuleDeclaration
                || node_store.kind(parent) == ast::Kind::EnumDeclaration)
                && node_store.name(parent) == Some(node)
        });

        let symbol =
            self.reference_get_referenced_value_symbol(node, start_in_declaration_container)?;
        let node_store = self.store_for_node(node);
        let reference_file = ast::get_source_file_of_node(node_store, Some(node));
        let current = node_store.parent(node);
        self.get_referenced_export_container_for_symbol(
            symbol,
            current,
            reference_file,
            prefix_locals,
        )
    }

    pub fn get_referenced_export_container_for_identifier_text(
        &mut self,
        location: ast::Node,
        name: &str,
        prefix_locals: bool,
    ) -> Option<ast::Node> /*SourceFile|ModuleDeclaration|EnumDeclaration*/ {
        if !ast::is_parse_tree_node(self.store_for_node(location), location) {
            return None;
        }

        let symbol = self
            .resolve_name(
                Some(location),
                name,
                ast::SYMBOL_FLAGS_EXPORT_VALUE | ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_ALIAS,
                None,  /* nameNotFoundMessage */
                false, /* isUse */
                false, /* excludeGlobals */
            )
            .map(SymbolIdentity::from_symbol_handle)?;
        let location_store = self.store_for_node(location);
        let reference_file = ast::get_source_file_of_node(location_store, Some(location));
        self.get_referenced_export_container_for_symbol(
            symbol,
            Some(location),
            reference_file,
            prefix_locals,
        )
    }

    fn get_referenced_export_container_for_symbol(
        &mut self,
        mut symbol: SymbolIdentity,
        mut current: Option<ast::Node>,
        reference_file: Option<ast::Node>,
        prefix_locals: bool,
    ) -> Option<ast::Node> /*SourceFile|ModuleDeclaration|EnumDeclaration*/ {
        if self
            .symbol_identity_flags_for_emit(symbol)
            .intersects(ast::SYMBOL_FLAGS_EXPORT_VALUE)
        {
            let export_symbol = self
                .symbol_identity_export_symbol_for_emit(symbol)
                .and_then(|export_symbol| self.reference_get_merged_symbol(Some(export_symbol)))?;
            if !prefix_locals
                && self
                    .symbol_identity_flags_for_emit(export_symbol)
                    .intersects(ast::SYMBOL_FLAGS_EXPORT_HAS_LOCAL)
                && !self
                    .symbol_identity_flags_for_emit(export_symbol)
                    .intersects(ast::SYMBOL_FLAGS_VARIABLE)
            {
                return None;
            }
            symbol = export_symbol;
        }

        let parent_symbol = self.reference_get_parent_of_symbol(Some(symbol))?;
        if self
            .symbol_identity_flags_for_emit(parent_symbol)
            .intersects(ast::SYMBOL_FLAGS_VALUE_MODULE)
            && self
                .symbol_identity_value_declaration_for_emit(parent_symbol)
                .as_ref()
                .is_some_and(|declaration| {
                    self.store_for_node(*declaration).kind(*declaration) == ast::Kind::SourceFile
                })
        {
            let symbol_file_node =
                self.symbol_identity_value_declaration_for_emit(parent_symbol)?;
            let symbol_store = self.store_for_node(symbol_file_node);
            let symbol_file = ast::get_source_file_of_node(symbol_store, Some(symbol_file_node))?;
            if reference_file != Some(symbol_file) {
                return None;
            }
            return Some(symbol_file_node);
        }

        while let Some(ancestor) = current {
            let ancestor_store = self.store_for_node(ancestor);
            let ancestor_kind = ancestor_store.kind(ancestor);
            let next_current = ancestor_store.parent(ancestor);
            let is_matching_container = (ancestor_kind == ast::Kind::ModuleDeclaration
                || ancestor_kind == ast::Kind::EnumDeclaration)
                && self
                    .reference_get_symbol_of_declaration(Some(ancestor))
                    .as_ref()
                    .is_some_and(|symbol| self.same_symbol_identity(*symbol, parent_symbol));
            if is_matching_container {
                return Some(ancestor);
            }
            current = next_current;
        }
        None
    }

    pub fn set_referenced_import_declaration(
        &self,
        node: ast::IdentifierNode,
        ref_: ast::Declaration,
    ) {
        self.semantic_state.set_jsx_import_ref(node, Some(ref_));
    }

    pub fn get_referenced_import_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return self.semantic_state.jsx_import_ref(node);
        }

        let symbol = self.get_referenced_value_or_alias_symbol(node);
        let is_non_local_alias = symbol.is_some_and(|symbol| {
            let handle = symbol.symbol_handle();
            let flags = self.symbol_handle_flags(handle);
            flags & (ast::SYMBOL_FLAGS_ALIAS | ast::SYMBOL_FLAGS_VALUE) == ast::SYMBOL_FLAGS_ALIAS
                || flags & ast::SYMBOL_FLAGS_ALIAS != ast::SYMBOL_FLAGS_NONE
                    && flags & ast::SYMBOL_FLAGS_ASSIGNMENT != ast::SYMBOL_FLAGS_NONE
        });
        if is_non_local_alias {
            let symbol = symbol.unwrap();
            if self
                .get_type_only_alias_declaration_identity_for_emit(symbol, ast::SYMBOL_FLAGS_VALUE)
                .is_some()
            {
                return None;
            }
            return self.get_declaration_of_alias_symbol_handle(symbol.symbol_handle());
        }
        None
    }

    pub fn get_referenced_value_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return None;
        }

        let symbol = self.reference_get_referenced_value_symbol(
            node, false, /* startInDeclarationContainer */
        )?;
        self.reference_get_export_symbol_of_value_symbol_if_exported(Some(symbol))
            .and_then(|symbol| self.symbol_identity_value_declaration_for_emit(symbol))
    }

    pub fn get_referenced_value_declarations(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Vec<ast::Declaration> {
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return Vec::new();
        }

        let mut declarations = Vec::new();
        let Some(symbol) = self.reference_get_referenced_value_symbol(
            node, false, /* startInDeclarationContainer */
        ) else {
            return declarations;
        };
        let Some(symbol) =
            self.reference_get_export_symbol_of_value_symbol_if_exported(Some(symbol))
        else {
            return declarations;
        };

        self.for_each_symbol_handle_declaration(symbol.symbol_handle(), |checker, declaration| {
            let declaration_store = checker.store_for_node(declaration);
            match declaration_store.kind(declaration) {
                ast::Kind::VariableDeclaration
                | ast::Kind::Parameter
                | ast::Kind::BindingElement
                | ast::Kind::PropertyDeclaration
                | ast::Kind::PropertyAssignment
                | ast::Kind::ShorthandPropertyAssignment
                | ast::Kind::EnumMember
                | ast::Kind::ObjectLiteralExpression
                | ast::Kind::FunctionDeclaration
                | ast::Kind::FunctionExpression
                | ast::Kind::ArrowFunction
                | ast::Kind::ClassDeclaration
                | ast::Kind::ClassExpression
                | ast::Kind::EnumDeclaration
                | ast::Kind::MethodDeclaration
                | ast::Kind::GetAccessor
                | ast::Kind::SetAccessor
                | ast::Kind::ModuleDeclaration => {
                    declarations.push(declaration);
                }
                _ => {}
            }
        });
        declarations
    }

    pub fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String {
        self.reference_get_element_access_expression_name(expression)
    }

    pub fn get_referenced_member_value_declaration(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Declaration> {
        if !ast::is_parse_tree_node(self.store_for_node(node), node) {
            return None;
        }

        let mut symbol = self.reference_get_resolved_symbol(Some(node));
        let node_symbol = self.get_symbol_of_node(node);
        if symbol.is_none() && node_symbol.is_some() {
            symbol = self
                .reference_get_merged_symbol(node_symbol.map(SymbolIdentity::from_symbol_handle));
        }
        let symbol = symbol?;
        self.reference_get_export_symbol_of_value_symbol_if_exported(Some(symbol))
            .and_then(|symbol| self.symbol_identity_value_declaration_for_emit(symbol))
    }

    // TODO: the emit resolver being responsible for some amount of node construction crosses layering boundaries,
    // and requires giving it access to a lot of context it's otherwise not required to have, which also further complicates the API
    // and likely reduces performance. There's probably some refactoring that could be done here to simplify this.

    pub fn create_return_type_of_signature_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        signature_declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> ast::Node {
        let original = emit_context.parse_node(&signature_declaration);
        let Some(original) = original else {
            return emit_context
                .factory
                .new_keyword_type_node(ast::Kind::AnyKeyword);
        };
        let original = original;
        let result = {
            let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
            request_node_builder.serialize_return_type_for_signature(
                original,
                Some(enclosing_declaration.clone()),
                flags,
                internal_flags,
                Some(tracker),
            )
        };
        result.unwrap_or_else(|| {
            emit_context
                .factory
                .new_keyword_type_node(ast::Kind::AnyKeyword)
        })
    }

    pub fn create_type_parameters_of_signature_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        signature_declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        let original = emit_context.parse_node(&signature_declaration);
        let Some(original) = original else {
            return Vec::new();
        };
        let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
        let original = original;
        request_node_builder
            .serialize_type_parameters_for_signature(
                original,
                Some(enclosing_declaration.clone()),
                flags,
                internal_flags,
                Some(tracker),
            )
            .into_iter()
            .collect()
    }

    pub fn create_type_of_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> ast::Node {
        let original = emit_context.parse_node(&declaration);
        let Some(original) = original else {
            return emit_context
                .factory
                .new_keyword_type_node(ast::Kind::AnyKeyword);
        };
        let original = original;
        let symbol = self.get_symbol_of_declaration(original);
        let Some(symbol) = symbol else {
            return emit_context
                .factory
                .new_keyword_type_node(ast::Kind::AnyKeyword);
        };
        let symbol_identity = SymbolIdentity::from_symbol_handle(symbol);
        let result = {
            let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
            request_node_builder.serialize_type_for_declaration(
                original,
                symbol_identity,
                Some(enclosing_declaration),
                flags | nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS,
                internal_flags,
                Some(tracker),
            )
        };
        result.unwrap_or_else(|| {
            emit_context
                .factory
                .new_keyword_type_node(ast::Kind::AnyKeyword)
        })
    }

    pub fn create_signature_declaration_with_synthetic_rest_parameter(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        kind: ast::Kind,
        modifiers: Vec<ast::Node>,
        name: Option<ast::Node>,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        let original = emit_context.parse_node(&declaration)?;
        let store = self.store_for_node(original);
        if !ast::is_in_js_file(store, original) {
            return None;
        }
        let source_parameter_count = store
            .source_parameters(original)
            .map(|parameters| parameters.len())
            .unwrap_or(0);
        let signature = self.get_signature_from_declaration(original);
        if !self.signature_has_rest_parameter(signature)
            || self.signature_parameter_identities(signature).len() <= source_parameter_count
        {
            return None;
        }
        let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
        request_node_builder.signature_to_signature_declaration_with_options(
            signature,
            kind,
            Some(enclosing_declaration),
            flags | nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS,
            internal_flags,
            Some(tracker),
            modifiers,
            name,
            None,
        )
    }

    pub fn get_declaration_statements_for_source_file(
        &mut self,
        emit_context: &mut printer::EmitContext,
        source_file: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        let Some(original) = emit_context.parse_node(&source_file) else {
            return Vec::new();
        };
        let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
        request_node_builder.get_declaration_statements_for_source_file(
            original,
            flags,
            internal_flags,
            Some(tracker),
        )
    }

    pub fn create_literal_const_value(
        &mut self,
        emit_context: &mut printer::EmitContext,
        node: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        let node = emit_context.parse_node(&node).unwrap();
        let symbol = self.get_symbol_of_declaration(node).unwrap();
        let t = self.get_type_of_symbol_handle(symbol);
        let mut enum_result: Option<ast::Node> = None;
        if self.type_flags(t) & TYPE_FLAGS_ENUM_LIKE != 0 {
            let enum_symbol = self.type_symbol_identity(t).unwrap();
            let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
            enum_result = request_node_builder.symbol_to_expression(
                enum_symbol,
                ast::SYMBOL_FLAGS_VALUE,
                Some(node),
                nodebuilder::FLAGS_NONE,
                nodebuilder::INTERNAL_FLAGS_NONE,
                Some(tracker),
            );
            // What about regularTrueType/regularFalseType - since those aren't fresh, we never make initializers from them
            // TODO: handle those if this function is ever used for more than initializers in declaration emit
        } else if t == self.semantic_state.semantic_handles().true_type {
            enum_result = Some(
                emit_context
                    .factory
                    .new_keyword_expression(ast::Kind::TrueKeyword),
            );
        } else if t == self.semantic_state.semantic_handles().false_type {
            enum_result = Some(
                emit_context
                    .factory
                    .new_keyword_expression(ast::Kind::FalseKeyword),
            );
        }
        if enum_result.is_some() {
            return enum_result;
        }
        if self.type_flags(t) & TYPE_FLAGS_LITERAL == 0 {
            return None; // non-literal type
        }
        match self.type_record(t).as_literal_type().value.clone() {
            LiteralValue::String(value) => Some(
                emit_context
                    .factory
                    .new_string_literal(&value, ast::TokenFlags::None),
            ),
            LiteralValue::Number(value) => {
                if value.0.abs() != value.0 {
                    // negative
                    let operand = emit_context
                        .factory
                        .new_numeric_literal(&value.to_string()[1..], ast::TokenFlags::None);
                    return Some(
                        emit_context
                            .factory
                            .new_prefix_unary_expression(ast::Kind::MinusToken, operand),
                    );
                }
                Some(
                    emit_context
                        .factory
                        .new_numeric_literal(&value.to_string(), ast::TokenFlags::None),
                )
            }
            LiteralValue::BigInt(value) | LiteralValue::PseudoBigInt(value) => {
                Some(emit_context.factory.new_big_int_literal(
                    &(pseudo_big_int_to_string(value) + "n"),
                    ast::TokenFlags::None,
                ))
            }
            LiteralValue::Bool(value) => {
                let mut kind = ast::Kind::FalseKeyword;
                if value {
                    kind = ast::Kind::TrueKeyword;
                }
                Some(emit_context.factory.new_keyword_expression(kind))
            }
            _ => panic!("unhandled literal const value kind"),
        }
    }

    pub fn create_type_of_expression(
        &mut self,
        emit_context: &mut printer::EmitContext,
        expression: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> ast::Node {
        let expression = emit_context.parse_node(&expression);
        let Some(expression) = expression else {
            return emit_context
                .factory
                .new_keyword_type_node(ast::Kind::AnyKeyword);
        };
        let enclosing_declaration = emit_context
            .parse_node(&enclosing_declaration)
            .unwrap_or(enclosing_declaration);
        let expression = expression;
        let result = {
            let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
            request_node_builder.serialize_type_for_expression(
                expression,
                Some(enclosing_declaration.clone()),
                flags | nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS,
                internal_flags,
                Some(tracker),
            )
        };
        result.unwrap_or_else(|| {
            emit_context
                .factory
                .new_keyword_type_node(ast::Kind::AnyKeyword)
        })
    }

    pub fn create_late_bound_index_signatures(
        &mut self,
        emit_context: &mut printer::EmitContext,
        container: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        let container = emit_context.parse_node(&container).unwrap();
        let tracker = Rc::new(RefCell::new(tracker));

        let sym =
            SymbolIdentity::from_symbol_handle(self.get_symbol_of_declaration(container).unwrap());
        let type_of_symbol = self.get_type_of_symbol_identity(sym);
        let static_infos = self.get_index_infos_of_type(type_of_symbol);
        let member_identities = self.collect_members_of_symbol_identities(sym);
        let instance_index_symbol = member_identities
            .get(ast::INTERNAL_SYMBOL_NAME_INDEX)
            .copied();
        let mut instance_infos: Vec<IndexInfoHandle> = Vec::new();
        if let Some(instance_index_symbol) = instance_index_symbol {
            let sibling_symbol_identities: Vec<_> = member_identities.values().copied().collect();
            instance_infos = self.get_index_infos_of_index_symbol_identity_for_emit(
                instance_index_symbol,
                sibling_symbol_identities,
            );
        }

        let mut result = Vec::new();
        for (i, info_list) in [static_infos, instance_infos].iter().enumerate() {
            let mut is_static = true;
            if i > 0 {
                is_static = false;
            }
            if info_list.is_empty() {
                continue;
            }
            for info in info_list {
                let info = *info;
                let info_record = self.index_info_record(info).clone();
                if info_record.declaration.is_some() {
                    continue;
                }
                if info
                    == self
                        .semantic_state
                        .semantic_handles()
                        .any_base_type_index_info
                {
                    continue; // inherited, but looks like a late-bound signature because it has no declarations
                }
                if !info_record.components.is_empty() {
                    // !!! TODO: Complete late-bound index info support - getObjectLiteralIndexInfo does not yet add late bound components to index signatures
                    let mut all_component_computed_names_serializable = true;
                    for c in &info_record.components {
                        let c = *c;
                        let c_store = self.store_for_node(c);
                        let Some(name) = c_store.name(c) else {
                            all_component_computed_names_serializable = false;
                            break;
                        };
                        let name_store = self.store_for_node(name);
                        let Some(expression) = name_store.expression(name) else {
                            all_component_computed_names_serializable = false;
                            break;
                        };
                        if !ast::is_computed_property_name(name_store, name)
                            || !ast::is_entity_name_expression(name_store, expression)
                            || self
                                .is_entity_name_visible_worker(
                                    expression,
                                    enclosing_declaration,
                                    false,
                                )
                                .accessibility
                                != printer::SymbolAccessibility::Accessible
                        {
                            all_component_computed_names_serializable = false;
                            break;
                        }
                    }
                    if all_component_computed_names_serializable {
                        for c in &info_record.components {
                            let c = *c;
                            if self.has_late_bindable_name(c) {
                                // skip late bound props that contribute to the index signature - they'll be preserved via other means
                                continue;
                            }

                            let c_store = self.store_for_node(c);
                            let name_node = c_store.name(c).unwrap();
                            let name_store = self.store_for_node(name_node);
                            let expression = name_store.expression(name_node).unwrap();
                            let first_identifier =
                                ast::get_first_identifier(name_store, expression).unwrap();
                            let first_identifier_text =
                                name_store.text(first_identifier).to_string();
                            let name = self.resolve_name(
                                Some(first_identifier),
                                &first_identifier_text,
                                ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_EXPORT_VALUE,
                                None,  /*nameNotFoundMessage*/
                                true,  /*isUse*/
                                false, /*excludeGlobals*/
                            );
                            if let Some(name) = name {
                                let symbol = SymbolIdentity::from_symbol_handle(name);
                                let symbol_flags = self.symbol_handle_flags(name);
                                if !tracker.borrow_mut().track_symbol(
                                    symbol.ast_identity(),
                                    symbol_flags,
                                    Some(enclosing_declaration.clone()),
                                    ast::SYMBOL_FLAGS_VALUE,
                                ) && !symbol_flags.intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
                                {
                                    let accessibility = self.is_symbol_accessible_public(
                                        Some(symbol),
                                        enclosing_declaration,
                                        ast::SYMBOL_FLAGS_VALUE,
                                        true, /*shouldComputeAliasToMarkVisible*/
                                    );
                                    match accessibility.accessibility {
                                        printer::SymbolAccessibility::Accessible => {
                                            tracker.borrow_mut().mark_aliases_visible(
                                                &accessibility.aliases_to_make_visible,
                                            );
                                        }
                                        printer::SymbolAccessibility::CannotBeNamed
                                        | printer::SymbolAccessibility::NotAccessible => {
                                            let accessibility_kind = if accessibility.accessibility
                                                == printer::SymbolAccessibility::CannotBeNamed
                                            {
                                                nodebuilder::SymbolAccessibility::CannotBeNamed
                                            } else {
                                                nodebuilder::SymbolAccessibility::NotAccessible
                                            };
                                            tracker.borrow_mut().report_symbol_accessibility_error(
                                                accessibility_kind,
                                                &accessibility.error_symbol_name,
                                                &accessibility.error_module_name,
                                                accessibility.error_node,
                                            );
                                        }
                                        printer::SymbolAccessibility::NotResolved => {}
                                    }
                                }
                            }

                            let type_node = {
                                let component_symbol = self.get_symbol_of_declaration(c).unwrap();
                                let symbol_type = self.get_type_of_symbol_handle(component_symbol);
                                let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
                                request_node_builder.type_to_type_node(
                                    symbol_type,
                                    Some(enclosing_declaration.clone()),
                                    flags,
                                    internal_flags,
                                    Some(Box::new(SharedSymbolTracker {
                                        inner: tracker.clone(),
                                    })),
                                )
                            };
                            let factory = &mut emit_context.factory;
                            let mut mods = if is_static {
                                vec![factory.new_modifier(ast::Kind::StaticKeyword)]
                            } else {
                                Vec::new()
                            };
                            if info_record.is_readonly {
                                mods.push(factory.new_modifier(ast::Kind::ReadonlyKeyword));
                            }
                            let modifiers = if !mods.is_empty() {
                                Some(factory.new_modifier_list(mods))
                            } else {
                                None
                            };
                            let c_store = self.store_for_node(c);
                            let name = factory
                                .node_factory
                                .deep_clone_node_from_store_preserve_location(
                                    c_store,
                                    c_store.name(c).unwrap(),
                                );
                            let question_token = c_store.question_token(c).map(|question_token| {
                                let question_token_store = self.store_for_node(question_token);
                                factory
                                    .node_factory
                                    .deep_clone_node_from_store_preserve_location(
                                        question_token_store,
                                        question_token,
                                    )
                            });
                            let decl = factory.new_property_declaration(
                                modifiers,
                                name,
                                question_token,
                                type_node,
                                None,
                            );
                            result.push(decl);
                        }
                        continue;
                    }
                }
                let node = {
                    let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
                    request_node_builder.index_info_to_index_signature_declaration(
                        info,
                        Some(enclosing_declaration.clone()),
                        flags,
                        internal_flags,
                        Some(Box::new(SharedSymbolTracker {
                            inner: tracker.clone(),
                        })),
                    )
                };
                let node = if let Some(existing) = node {
                    if is_static {
                        let mut mod_nodes =
                            vec![emit_context.factory.new_modifier(ast::Kind::StaticKeyword)];
                        mod_nodes.extend(emit_context.factory.store().modifier_nodes(existing));
                        let mods = emit_context.factory.new_modifier_list(mod_nodes);
                        let source_parameters = emit_context
                            .factory
                            .store()
                            .source_parameters(existing)
                            .expect("index signature should have parameters");
                        let parameters = emit_context
                            .factory
                            .new_node_list(source_parameters.nodes());
                        let type_node = emit_context.factory.store().r#type(existing);
                        Some(emit_context.factory.update_index_signature_declaration(
                            existing, mods, parameters, type_node,
                        ))
                    } else {
                        Some(existing)
                    }
                } else {
                    None
                };
                if let Some(node) = node {
                    result.push(node);
                }
            }
        }
        result
    }

    fn get_index_infos_of_index_symbol_identity_for_emit(
        &mut self,
        index_symbol: SymbolIdentity,
        sibling_symbols: Vec<SymbolIdentity>,
    ) -> Vec<IndexInfoHandle> {
        let mut index_infos = Vec::new();
        let mut has_computed_string_property = false;
        let mut has_computed_number_property = false;
        let mut has_computed_symbol_property = false;
        let mut readonly_computed_string_property = true;
        let mut readonly_computed_number_property = true;
        let mut readonly_computed_symbol_property = true;
        let mut property_symbols = Vec::new();
        self.for_each_symbol_handle_declaration(
            index_symbol.symbol_handle(),
            |checker, declaration| {
                let declaration_store = checker.store_for_node(declaration);
                if ast::is_index_signature_declaration(declaration_store, declaration) {
                    let parameters = declaration_store
                        .parameters(declaration)
                        .map(|parameters| parameters.iter().collect::<Vec<_>>())
                        .unwrap_or_default();
                    let return_type_node = declaration_store.r#type(declaration);
                    if parameters.len() == 1 {
                        let parameter_store = checker.store_for_node(parameters[0]);
                        let type_node = parameter_store.r#type(parameters[0]);
                        if let Some(type_node) = type_node {
                            let mut value_type = checker.semantic_state.semantic_handles().any_type;
                            if let Some(return_type_node) = return_type_node {
                                value_type = checker.get_type_from_type_node(return_type_node);
                            }
                            let type_node_type = checker.get_type_from_type_node(type_node);
                            let key_types =
                                if checker.type_flags(type_node_type) & TYPE_FLAGS_UNION != 0 {
                                    checker.type_types(type_node_type)
                                } else {
                                    vec![type_node_type]
                                };
                            for key_type in key_types {
                                if checker.is_valid_index_key_type(key_type)
                                    && find_index_info(checker, &index_infos, key_type).is_none()
                                {
                                    let index_info = checker.new_index_info(
                                        key_type,
                                        value_type,
                                        ast::has_modifier(
                                            declaration_store,
                                            declaration,
                                            ast::ModifierFlags::READONLY,
                                        ),
                                        Some(declaration),
                                        None,
                                    );
                                    index_infos.push(index_info);
                                }
                            }
                        }
                    }
                } else if checker.has_late_bindable_index_signature(declaration) {
                    let decl_name = if ast::is_binary_expression(declaration_store, declaration) {
                        declaration_store.left(declaration).unwrap()
                    } else {
                        declaration_store.name(declaration).unwrap()
                    };
                    let key_type = if ast::is_element_access_expression(
                        checker.store_for_node(decl_name),
                        decl_name,
                    ) {
                        let decl_name_store = checker.store_for_node(decl_name);
                        checker.check_expression_cached(
                            decl_name_store.argument_expression(decl_name).unwrap(),
                        )
                    } else {
                        checker.check_computed_property_name(decl_name)
                    };
                    if find_index_info(checker, &index_infos, key_type).is_some() {
                        return;
                    }
                    if checker.is_type_assignable_to(
                        key_type,
                        checker
                            .semantic_state
                            .semantic_handles()
                            .string_number_symbol_type,
                    ) {
                        if checker.is_type_assignable_to(
                            key_type,
                            checker.semantic_state.semantic_handles().number_type,
                        ) {
                            has_computed_number_property = true;
                            if !crate::utilities::has_readonly_modifier(
                                declaration_store,
                                declaration,
                            ) {
                                readonly_computed_number_property = false;
                            }
                        } else if checker.is_type_assignable_to(
                            key_type,
                            checker.semantic_state.semantic_handles().es_symbol_type,
                        ) {
                            has_computed_symbol_property = true;
                            if !crate::utilities::has_readonly_modifier(
                                declaration_store,
                                declaration,
                            ) {
                                readonly_computed_symbol_property = false;
                            }
                        } else {
                            has_computed_string_property = true;
                            if !crate::utilities::has_readonly_modifier(
                                declaration_store,
                                declaration,
                            ) {
                                readonly_computed_string_property = false;
                            }
                        }
                        property_symbols.push(SymbolIdentity::from_symbol_handle(
                            checker.node_symbol(declaration).unwrap(),
                        ));
                    }
                }
            },
        );
        if has_computed_string_property
            || has_computed_number_property
            || has_computed_symbol_property
        {
            for sym in sibling_symbols {
                if sym != index_symbol {
                    property_symbols.push(sym);
                }
            }
            if has_computed_string_property
                && find_index_info(
                    self,
                    &index_infos,
                    self.semantic_state.semantic_handles().string_type,
                )
                .is_none()
            {
                index_infos.push(self.get_object_literal_index_info_from_identities(
                    readonly_computed_string_property,
                    &property_symbols,
                    self.semantic_state.semantic_handles().string_type,
                ));
            }
            if has_computed_number_property
                && find_index_info(
                    self,
                    &index_infos,
                    self.semantic_state.semantic_handles().number_type,
                )
                .is_none()
            {
                index_infos.push(self.get_object_literal_index_info_from_identities(
                    readonly_computed_number_property,
                    &property_symbols,
                    self.semantic_state.semantic_handles().number_type,
                ));
            }
            if has_computed_symbol_property
                && find_index_info(
                    self,
                    &index_infos,
                    self.semantic_state.semantic_handles().es_symbol_type,
                )
                .is_none()
            {
                index_infos.push(self.get_object_literal_index_info_from_identities(
                    readonly_computed_symbol_property,
                    &property_symbols,
                    self.semantic_state.semantic_handles().es_symbol_type,
                ));
            }
        }
        index_infos
    }

    pub fn get_effective_declaration_flags_for_emit(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierFlags {
        // node = emitContext.ParseNode(node)
        self.get_effective_declaration_flags(node.clone(), flags)
    }

    pub fn get_resolution_mode_override_for_emit(
        &mut self,
        node: ast::Node,
    ) -> core::ResolutionMode {
        // node = emitContext.ParseNode(node)
        self.get_resolution_mode_override(node.clone(), false)
    }

    pub fn get_constant_value_for_emit(&mut self, node: ast::Node) -> Option<evaluator::Value> {
        // node = emitContext.ParseNode(node)
        self.get_constant_value(node.clone())
    }

    pub fn get_type_reference_serialization_kind(
        &mut self,
        type_name: Option<ast::Node>,
        location: Option<ast::Node>,
    ) -> printer::TypeReferenceSerializationKind {
        // typeName = emitContext.ParseNode(typeName)
        // location = emitContext.ParseNode(location)

        let (Some(type_name), Some(location)) = (type_name, location) else {
            return printer::TypeReferenceSerializationKind::Unknown;
        };
        let type_name = type_name.clone();
        let location = location.clone();

        // Resolve the symbol as a value to ensure the type can be reached at runtime during emit.
        let mut is_type_only = false;
        if ast::is_qualified_name(self.store_for_node(type_name), type_name) {
            let type_name_store = self.store_for_node(type_name);
            let first_identifier = ast::get_first_identifier(type_name_store, type_name).unwrap();
            let root_value_symbol = self.resolve_entity_name(
                first_identifier,
                ast::SYMBOL_FLAGS_VALUE,
                true,
                true,
                Some(location),
            );

            if let Some(root_value_symbol) = root_value_symbol {
                is_type_only =
                    self.with_symbol_identity_declarations(root_value_symbol, |declarations| {
                        !declarations.is_empty()
                            && declarations.iter().all(|&declaration| {
                                let store = self.store_for_node(declaration);
                                ast::is_type_only_import_or_export_declaration(store, declaration)
                            })
                    });
            }
        }
        let value_symbol = self.resolve_entity_name(
            type_name,
            ast::SYMBOL_FLAGS_VALUE,
            true,
            true,
            Some(location),
        );
        let mut resolved_value_symbol = value_symbol;
        if let Some(value_symbol) = value_symbol {
            if self.symbol_identity_flags_for_emit(value_symbol) & ast::SYMBOL_FLAGS_ALIAS != 0 {
                resolved_value_symbol = Some(self.resolve_symbol_identity(value_symbol, false));
            }
        }

        is_type_only = is_type_only
            || value_symbol.is_some_and(|value_symbol| {
                self.get_type_only_alias_declaration_identity_for_emit(
                    value_symbol,
                    ast::SYMBOL_FLAGS_VALUE,
                )
                .is_some()
            });

        // Resolve the symbol as a type so that we can provide a more useful hint for the type serializer.
        let type_symbol = self.resolve_entity_name(
            type_name,
            ast::SYMBOL_FLAGS_TYPE,
            true,
            true,
            Some(location),
        );
        let mut resolved_type_symbol = type_symbol;
        if let Some(type_symbol) = type_symbol {
            if self.symbol_identity_flags_for_emit(type_symbol) & ast::SYMBOL_FLAGS_ALIAS != 0 {
                resolved_type_symbol = Some(self.resolve_symbol_identity(type_symbol, false));
            }
        }
        // In case the value symbol can't be resolved (e.g. because of missing declarations), use type symbol for reachability check.
        is_type_only = is_type_only
            || type_symbol.is_some_and(|type_symbol| {
                self.get_type_only_alias_declaration_identity_for_emit(
                    type_symbol,
                    ast::SYMBOL_FLAGS_TYPE,
                )
                .is_some()
            });

        if resolved_value_symbol.is_some()
            && self.same_optional_symbol_identity(resolved_value_symbol, resolved_type_symbol)
        {
            let global_promise_symbol = {
                let resolver = (self.semantic_state.get_global_promise_constructor_symbol).clone();
                self.resolve_global_symbol(resolver)
            };
            if global_promise_symbol.is_some()
                && self.same_optional_symbol_identity(resolved_value_symbol, global_promise_symbol)
            {
                return printer::TypeReferenceSerializationKind::Promise;
            }

            let constructor_type = self.get_type_of_symbol_identity(resolved_value_symbol.unwrap());
            if self.is_constructor_type(constructor_type) {
                if is_type_only {
                    return printer::TypeReferenceSerializationKind::TypeWithCallSignature;
                }
                return printer::TypeReferenceSerializationKind::TypeWithConstructSignatureAndValue;
            }
        }

        // We might not be able to resolve type symbol so use unknown type in that case (eg error case)
        let Some(resolved_type_symbol) = resolved_type_symbol else {
            if is_type_only {
                return printer::TypeReferenceSerializationKind::ObjectType;
            }
            return printer::TypeReferenceSerializationKind::Unknown;
        };

        let type_ = self.get_declared_type_of_symbol_identity_or_error(resolved_type_symbol);
        if self.is_error_type(type_) {
            if is_type_only {
                return printer::TypeReferenceSerializationKind::ObjectType;
            }
            return printer::TypeReferenceSerializationKind::Unknown;
        }

        if self.type_flags(type_) & TYPE_FLAGS_ANY_OR_UNKNOWN != 0 {
            printer::TypeReferenceSerializationKind::ObjectType
        } else if self.is_type_assignable_to_kind(
            type_,
            TYPE_FLAGS_VOID | TYPE_FLAGS_NULLABLE | TYPE_FLAGS_NEVER,
        ) {
            printer::TypeReferenceSerializationKind::VoidNullableOrNeverType
        } else if self.is_type_assignable_to_kind(type_, TYPE_FLAGS_BOOLEAN_LIKE) {
            printer::TypeReferenceSerializationKind::BooleanType
        } else if self.is_type_assignable_to_kind(type_, TYPE_FLAGS_NUMBER_LIKE) {
            printer::TypeReferenceSerializationKind::NumberLikeType
        } else if self.is_type_assignable_to_kind(type_, TYPE_FLAGS_BIG_INT_LIKE) {
            printer::TypeReferenceSerializationKind::BigIntLikeType
        } else if self.is_type_assignable_to_kind(type_, TYPE_FLAGS_STRING_LIKE) {
            printer::TypeReferenceSerializationKind::StringLikeType
        } else if self.is_tuple_type(type_) {
            printer::TypeReferenceSerializationKind::ArrayLikeType
        } else if self.is_type_assignable_to_kind(type_, TYPE_FLAGS_ES_SYMBOL_LIKE) {
            printer::TypeReferenceSerializationKind::ESSymbolType
        } else if self.is_function_type(type_) {
            printer::TypeReferenceSerializationKind::TypeWithCallSignature
        } else if self.is_array_type(type_) {
            printer::TypeReferenceSerializationKind::ArrayLikeType
        } else {
            printer::TypeReferenceSerializationKind::ObjectType
        }
    }

    pub fn get_properties_of_container_function(
        &mut self,
        node: Option<ast::Node>,
    ) -> Vec<SymbolIdentity> {
        // This is explicitly _not locked_ because it is only called via error reporters invoked via node builder calls
        // to the symbol tracker already within locked contexts.
        // r.checkerMu.Lock()
        // defer r.checkerMu.Unlock()
        let Some(node) = node else {
            return Vec::new();
        };
        let s = self.get_symbol_of_declaration(node);
        let Some(s) = s else {
            return Vec::new();
        };
        let type_of_symbol = self.get_type_of_symbol_handle(s);
        self.get_properties_of_type(type_of_symbol)
            .into_iter()
            .collect()
    }

    pub fn try_js_type_node_to_type_node(
        &mut self,
        emit_context: &mut printer::EmitContext,
        type_node: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        _tracker: &mut dyn nodebuilder::SymbolTracker,
    ) -> Option<ast::Node> {
        let type_node = emit_context.parse_node(&type_node).unwrap();

        let mut request_node_builder = new_node_builder(self, emit_context); // TODO: cache per-context
        request_node_builder.try_js_type_node_to_type_node(
            type_node,
            Some(enclosing_declaration.clone()),
            flags,
            internal_flags,
            None,
        )
    }
}

impl<'a, 'state> binder::BinderReferenceResolver for Checker<'a, 'state> {
    fn get_referenced_export_container(
        &mut self,
        node: ast::IdentifierNode,
        prefix_locals: bool,
    ) -> Option<ast::Node> {
        Self::get_referenced_export_container(self, node, prefix_locals)
    }

    fn get_referenced_import_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        Self::get_referenced_import_declaration(self, node)
    }

    fn get_referenced_value_declaration(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Option<ast::Declaration> {
        Self::get_referenced_value_declaration(self, node)
    }

    fn get_referenced_value_declarations(
        &mut self,
        node: ast::IdentifierNode,
    ) -> Vec<ast::Declaration> {
        Self::get_referenced_value_declarations(self, node)
    }

    fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String {
        self.reference_get_element_access_expression_name(expression)
    }

    fn get_referenced_member_value_declaration(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Declaration> {
        Self::get_referenced_member_value_declaration(self, node)
    }
}

impl<'a, 'state> printer::EmitResolver for Checker<'a, 'state> {
    fn source_file_store(&self, node: ast::Node) -> Option<&ast::AstStore> {
        self.try_store_for_node(node)
    }

    fn is_referenced_alias_declaration(&mut self, node: ast::Node) -> bool {
        self.is_referenced_alias_declaration(node)
    }

    fn is_value_alias_declaration(&mut self, node: ast::Node) -> bool {
        self.is_value_alias_declaration_public(node)
    }

    fn is_top_level_value_import_equals_with_entity_name(&mut self, node: ast::Node) -> bool {
        self.is_top_level_value_import_equals_with_entity_name(node)
    }

    fn mark_linked_references_recursively(&mut self, file: &ast::SourceFile) {
        self.mark_linked_references_recursively(Some(file));
    }

    fn get_external_module_file_from_declaration(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::SourceFile> {
        self.get_external_module_file_from_declaration_for_emit(node)
    }

    fn get_effective_declaration_flags(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierFlags {
        self.get_effective_declaration_flags_for_emit(node, flags)
    }

    fn get_resolution_mode_override(&mut self, node: ast::Node) -> core::ResolutionMode {
        self.get_resolution_mode_override_for_emit(node)
    }

    fn get_type_reference_serialization_kind(
        &mut self,
        type_name: ast::Node,
        serial_scope: ast::Node,
    ) -> printer::TypeReferenceSerializationKind {
        self.get_type_reference_serialization_kind(Some(type_name), Some(serial_scope))
    }

    fn get_constant_value(&mut self, node: ast::Node) -> Option<evaluator::Value> {
        self.get_constant_value_for_emit(node)
    }

    fn get_jsx_factory_entity(&mut self, location: ast::Node) -> Option<ast::Node> {
        self.get_jsx_factory_entity_for_emit(location)
    }

    fn get_jsx_fragment_factory_entity(&mut self, location: ast::Node) -> Option<ast::Node> {
        self.get_jsx_fragment_factory_entity_for_emit(location)
    }

    fn get_jsx_factory_entity_text(&mut self, location: ast::Node) -> Option<String> {
        self.get_jsx_factory_entity_text_for_emit(location)
    }

    fn get_jsx_fragment_factory_entity_text(&mut self, location: ast::Node) -> Option<String> {
        self.get_jsx_fragment_factory_entity_text_for_emit(location)
    }

    fn get_referenced_export_container_for_identifier_text(
        &mut self,
        location: ast::Node,
        name: &str,
        prefix_locals: bool,
    ) -> Option<ast::Node> {
        Self::get_referenced_export_container_for_identifier_text(
            self,
            location,
            name,
            prefix_locals,
        )
    }

    fn set_referenced_import_declaration(
        &self,
        node: ts_ast::Node,
        ref_declaration: ast::Declaration,
    ) {
        Self::set_referenced_import_declaration(self, node, ref_declaration)
    }

    fn precalculate_declaration_emit_visibility(&mut self, file: &ast::SourceFile) {
        self.precalculate_declaration_emit_visibility(file)
    }

    fn is_symbol_accessible(
        &mut self,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: ast::Node,
        meaning: ast::SymbolFlags,
        should_compute_alias_to_mark_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        self.is_symbol_accessible_public(
            Some(SymbolIdentity::from_symbol_handle(symbol.symbol_handle())),
            enclosing_declaration,
            meaning,
            should_compute_alias_to_mark_visible,
        )
    }

    fn is_entity_name_visible(
        &mut self,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
    ) -> printer::SymbolAccessibilityResult {
        self.is_entity_name_visible(entity_name, enclosing_declaration)
    }

    fn is_expando_function_declaration(&mut self, node: ast::Node) -> bool {
        self.is_expando_function_declaration(node)
    }

    fn is_expando_function_declaration_unsafe(&mut self, node: ast::Node) -> bool {
        self.is_expando_function_declaration_unsafe(node)
    }

    fn should_emit_function_properties(&mut self, node: ast::Node) -> bool {
        self.should_emit_function_properties(node)
    }

    fn get_symbol_name(&mut self, symbol: ast::SymbolIdentity) -> String {
        self.missing_name_symbol_identity_name(SymbolIdentity::from_symbol_handle(
            symbol.symbol_handle(),
        ))
    }

    fn get_symbol_value_declaration(&mut self, symbol: ast::SymbolIdentity) -> Option<ast::Node> {
        self.missing_name_symbol_identity_value_declaration(SymbolIdentity::from_symbol_handle(
            symbol.symbol_handle(),
        ))
    }

    fn set_expando_namespace_metadata(
        &mut self,
        synthesized_namespace: ast::Node,
        declaration: ast::Node,
        properties: &[ast::SymbolIdentity],
    ) {
        self.set_expando_namespace_metadata(synthesized_namespace, declaration, properties);
    }

    fn is_literal_const_declaration(&mut self, node: ast::Node) -> bool {
        self.is_literal_const_declaration(node)
    }

    fn requires_adding_implicit_undefined_with_symbol(
        &mut self,
        node: ast::Node,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        self.requires_adding_implicit_undefined(
            node,
            Some(SymbolIdentity::from_symbol_handle(symbol.symbol_handle())),
            enclosing_declaration,
        )
    }

    fn requires_adding_implicit_undefined(
        &mut self,
        node: ast::Node,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        self.requires_adding_implicit_undefined(node, None, enclosing_declaration)
    }

    fn is_declaration_visible(&mut self, node: ast::Node) -> bool {
        self.is_declaration_visible_public(node)
    }

    fn is_import_required_by_augmentation(&mut self, decl: ast::Node) -> bool {
        self.is_import_required_by_augmentation(decl)
    }

    fn is_definitely_reference_to_global_symbol_object(&mut self, node: ast::Node) -> bool {
        self.is_definitely_reference_to_global_symbol_object(node)
    }

    fn is_implementation_of_overload(&mut self, node: ast::Node) -> bool {
        self.is_implementation_of_overload(node)
    }

    fn is_first_declaration_of_symbol(&mut self, node: ast::Node) -> bool {
        self.is_first_declaration_of_symbol(node)
    }

    fn is_assignment_declaration(&mut self, node: ast::Node) -> bool {
        self.node_symbol(node).is_some_and(|symbol| {
            self.symbol_handle_flags(symbol)
                .intersects(ast::SYMBOL_FLAGS_ASSIGNMENT)
        })
    }

    fn is_common_js_alias_export(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        ast::is_binary_expression(store, node)
            && store
                .right(node)
                .is_some_and(|right| ast::is_identifier(self.store_for_node(right), right))
            && self.node_symbol(node).is_some_and(|symbol| {
                self.with_symbol_handle_declarations(symbol, |declarations| declarations.len() == 1)
            })
    }

    fn get_element_access_expression_name(&mut self, expression: ast::Node) -> String {
        self.get_element_access_expression_name(expression)
    }

    fn get_enum_member_value(&mut self, node: ast::Node) -> evaluator::Result {
        self.get_enum_member_value_for_emit(node)
    }

    fn is_late_bound(&mut self, node: ast::Node) -> bool {
        self.is_late_bound(Some(node))
    }

    fn is_optional_parameter(&mut self, node: ast::Node) -> bool {
        self.is_optional_parameter_public(node)
    }

    fn get_properties_of_container_function(
        &mut self,
        node: ast::Node,
    ) -> Vec<ast::SymbolIdentity> {
        self.get_properties_of_container_function(Some(node))
            .into_iter()
            .map(SymbolIdentity::ast_identity)
            .collect()
    }

    fn requires_adding_implicit_undefined_unsafe(
        &mut self,
        node: ast::Node,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
    ) -> bool {
        self.requires_adding_implicit_undefined_unsafe(
            node,
            Some(SymbolIdentity::from_symbol_handle(symbol.symbol_handle())),
            enclosing_declaration,
        )
    }

    fn create_type_of_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        Some(self.create_type_of_declaration(
            emit_context,
            declaration,
            enclosing_declaration,
            flags,
            internal_flags,
            tracker,
        ))
    }

    fn get_declaration_statements_for_source_file(
        &mut self,
        emit_context: &mut printer::EmitContext,
        source_file: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        self.get_declaration_statements_for_source_file(
            emit_context,
            source_file,
            flags,
            internal_flags,
            tracker,
        )
    }

    fn create_return_type_of_signature_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        signature_declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        Some(self.create_return_type_of_signature_declaration(
            emit_context,
            signature_declaration,
            enclosing_declaration,
            flags,
            internal_flags,
            tracker,
        ))
    }

    fn create_signature_declaration_with_synthetic_rest_parameter(
        &mut self,
        emit_context: &mut printer::EmitContext,
        declaration: ast::Node,
        kind: ast::Kind,
        modifiers: Vec<ast::Node>,
        name: Option<ast::Node>,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        self.create_signature_declaration_with_synthetic_rest_parameter(
            emit_context,
            declaration,
            kind,
            modifiers,
            name,
            enclosing_declaration,
            flags,
            internal_flags,
            tracker,
        )
    }

    fn create_type_parameters_of_signature_declaration(
        &mut self,
        emit_context: &mut printer::EmitContext,
        signature_declaration: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        self.create_type_parameters_of_signature_declaration(
            emit_context,
            signature_declaration,
            enclosing_declaration,
            flags,
            internal_flags,
            tracker,
        )
    }

    fn create_literal_const_value(
        &mut self,
        emit_context: &mut printer::EmitContext,
        node: ast::Node,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        self.create_literal_const_value(emit_context, node, tracker)
    }

    fn create_type_of_expression(
        &mut self,
        emit_context: &mut printer::EmitContext,
        expression: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Option<ast::Node> {
        Some(self.create_type_of_expression(
            emit_context,
            expression,
            enclosing_declaration,
            flags,
            internal_flags,
            tracker,
        ))
    }

    fn create_late_bound_index_signatures(
        &mut self,
        emit_context: &mut printer::EmitContext,
        container: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: Box<dyn nodebuilder::SymbolTracker>,
    ) -> Vec<ast::Node> {
        self.create_late_bound_index_signatures(
            emit_context,
            container,
            enclosing_declaration,
            flags,
            internal_flags,
            tracker,
        )
    }

    fn try_js_type_node_to_type_node(
        &mut self,
        emit_context: &mut printer::EmitContext,
        type_node: ast::Node,
        enclosing_declaration: ast::Node,
        flags: nodebuilder::Flags,
        internal_flags: nodebuilder::InternalFlags,
        tracker: &mut dyn nodebuilder::SymbolTracker,
    ) -> Option<ast::Node> {
        self.try_js_type_node_to_type_node(
            emit_context,
            type_node,
            enclosing_declaration,
            flags,
            internal_flags,
            tracker,
        )
    }
}
