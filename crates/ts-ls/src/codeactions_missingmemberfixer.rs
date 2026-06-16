use ts_ast as ast;
use ts_checker as checker;
use ts_collections::{FastHashMap as HashMap, FastHashMapExt};
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_nodebuilder as nodebuilder;

use crate::autoimport;
use crate::change;
use crate::lsutil;

fn synthetic_node_list(
    factory: &mut ast::NodeFactory,
    nodes: impl IntoIterator<Item = ast::Node>,
) -> ast::NodeList {
    factory.new_node_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        nodes,
    )
}

fn synthetic_modifier_list(
    factory: &mut ast::NodeFactory,
    modifiers: impl IntoIterator<Item = ast::Node>,
    flags: ast::ModifierFlags,
) -> ast::ModifierList {
    factory.new_modifier_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        modifiers,
        flags,
    )
}

fn output_node(
    import_state: &mut ast::AstImportState,
    source_store: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: ast::Node,
) -> ast::Node {
    if node.store_id() == factory.store().store_id() {
        return node;
    }
    assert_eq!(
        node.store_id(),
        source_store.store_id(),
        "missing member source node belongs to an unexpected AST store"
    );
    import_state.preserve_node(source_store, factory, node)
}

fn optional_output_node(
    import_state: &mut ast::AstImportState,
    source_store: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: Option<ast::Node>,
) -> Option<ast::Node> {
    node.map(|node| output_node(import_state, source_store, factory, node))
}

fn optional_output_modifiers(
    import_state: &mut ast::AstImportState,
    source_store: &ast::AstStore,
    factory: &mut ast::NodeFactory,
    node: ast::Node,
) -> Option<ast::ModifierList> {
    import_state
        .preserve_optional_source_modifier_list(factory, source_store.source_modifiers(node))
}

fn output_node_list_from_source(
    import_state: &mut ast::AstImportState,
    factory: &mut ast::NodeFactory,
    source_list: ast::SourceNodeList<'_>,
    nodes: impl IntoIterator<Item = ast::Node>,
) -> ast::NodeList {
    if source_list.is_missing() {
        return factory.new_missing_node_list(source_list.loc(), source_list.range());
    }

    let source_store = source_list.store();
    let output_nodes = nodes
        .into_iter()
        .map(|node| output_node(import_state, source_store, factory, node))
        .collect::<Vec<_>>();
    factory.new_node_list_with_trailing_comma(
        source_list.loc(),
        source_list.range(),
        output_nodes,
        source_list.has_trailing_comma(),
    )
}

fn optional_identifier_output_node(
    import_state: &mut ast::AstImportState,
    factory: &mut ast::NodeFactory,
    node: Option<&ast::Node>,
    source_store: Option<&ast::AstStore>,
) -> Option<ast::Node> {
    let node = *node?;
    if node.store_id() == factory.store().store_id() {
        return ast::is_identifier(factory.store(), node).then_some(node);
    }
    let source_store = source_store.expect("identifier name should have a source arena");
    assert_eq!(
        node.store_id(),
        source_store.store_id(),
        "identifier name belongs to an unexpected AST store"
    );
    if ast::is_identifier(source_store, node) {
        return Some(output_node(import_state, source_store, factory, node));
    }
    None
}

pub type PreserveOptionalFlags = i32;

pub const PRESERVE_OPTIONAL_FLAGS_METHOD: PreserveOptionalFlags = 1 << 0;
pub const PRESERVE_OPTIONAL_FLAGS_PROPERTY: PreserveOptionalFlags = 1 << 1;
pub const PRESERVE_OPTIONAL_FLAGS_ALL: PreserveOptionalFlags =
    PRESERVE_OPTIONAL_FLAGS_METHOD | PRESERVE_OPTIONAL_FLAGS_PROPERTY;

fn get_declaration_modifier_flags_from_symbol_identity(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> ast::ModifierFlags {
    let flags = type_checker
        .symbol_flags_public(symbol)
        .unwrap_or(ast::SYMBOL_FLAGS_NONE);
    let check_flags = type_checker
        .symbol_check_flags_public(symbol)
        .unwrap_or(ast::CHECK_FLAGS_NONE);
    let Some(value_declaration) = type_checker.symbol_value_declaration_public(symbol) else {
        if check_flags & ast::CHECK_FLAGS_SYNTHETIC != 0 {
            let access_modifier = if check_flags & ast::CHECK_FLAGS_CONTAINS_PRIVATE != 0 {
                ast::ModifierFlags::Private
            } else if check_flags & ast::CHECK_FLAGS_CONTAINS_PUBLIC != 0 {
                ast::ModifierFlags::Public
            } else {
                ast::ModifierFlags::Protected
            };
            let static_modifier = if check_flags & ast::CHECK_FLAGS_CONTAINS_STATIC != 0 {
                ast::ModifierFlags::Static
            } else {
                ast::ModifierFlags::None
            };
            return access_modifier | static_modifier;
        }
        if flags & ast::SYMBOL_FLAGS_PROTOTYPE != 0 {
            return ast::ModifierFlags::Public | ast::ModifierFlags::Static;
        }
        return ast::ModifierFlags::None;
    };

    let declarations = type_checker.collect_symbol_declarations_public(symbol);
    let declaration = if flags & ast::SYMBOL_FLAGS_GET_ACCESSOR != 0 {
        declarations
            .iter()
            .copied()
            .find(|declaration| {
                type_checker
                    .try_source_file_for_node_public(*declaration)
                    .is_some_and(|source_file| {
                        ast::is_get_accessor_declaration(source_file.store(), *declaration)
                    })
            })
            .unwrap_or(value_declaration)
    } else {
        value_declaration
    };
    let declaration_store = type_checker
        .try_source_file_for_node_public(declaration)
        .map(|source_file| source_file.store())
        .expect("symbol declaration should belong to a checker source file");
    let modifier_flags = ast::get_combined_modifier_flags(declaration_store, declaration);
    let parent_is_class = type_checker
        .symbol_parent_public(symbol)
        .and_then(|parent| type_checker.symbol_flags_public(parent))
        .is_some_and(|parent_flags| parent_flags & ast::SYMBOL_FLAGS_CLASS != 0);
    if parent_is_class {
        modifier_flags
    } else {
        modifier_flags & !ast::ModifierFlags::AccessibilityModifier
    }
}

pub struct MissingMemberFixer<'a, 'b, 'state> {
    pub change_tracker: &'b mut change::Tracker<'a>,
    pub type_checker: &'b mut checker::Checker<'a, 'state>,
    pub program: &'a compiler::Program,
    pub preferences: lsutil::UserPreferences,
    pub import_adder: Option<&'b mut autoimport::ImportAdder<'a>>,
    pub locale: locale::Locale,
}

pub fn new_missing_member_fixer<'a, 'b, 'state>(
    change_tracker: &'b mut change::Tracker<'a>,
    program: &'a compiler::Program,
    type_checker: &'b mut checker::Checker<'a, 'state>,
    preferences: lsutil::UserPreferences,
    import_adder: Option<&'b mut autoimport::ImportAdder<'a>>,
    locale: locale::Locale,
) -> MissingMemberFixer<'a, 'b, 'state> {
    MissingMemberFixer {
        change_tracker,
        type_checker,
        program,
        preferences,
        import_adder,
        locale,
    }
}

impl<'a, 'b, 'state> MissingMemberFixer<'a, 'b, 'state> {
    fn type_checker_mut(&mut self) -> &mut checker::Checker<'a, 'state> {
        self.type_checker
    }

    fn import_adder_mut(&mut self) -> Option<&mut autoimport::ImportAdder<'a>> {
        self.import_adder.as_deref_mut()
    }

    pub(crate) fn create_member_from_symbol(
        &mut self,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: &ast::Node,
        source_file: &'a ast::SourceFile,
        body: Option<&ast::Node>,
        preserve_optional: PreserveOptionalFlags,
    ) -> Result<Vec<ast::Node>, core::Error> {
        let declarations = self.type_checker.collect_symbol_declarations_public(symbol);
        let declaration = declarations.first();

        let quote_preference = lsutil::get_quote_preference(source_file, &self.preferences);
        let ambient = source_file.store().flags(*enclosing_declaration) & ast::NODE_FLAGS_AMBIENT
            != ast::NODE_FLAGS_NONE;
        let optional = self
            .type_checker
            .symbol_flags_public(symbol)
            .is_some_and(|flags| flags & ast::SYMBOL_FLAGS_OPTIONAL != 0);
        let mut kind = ast::Kind::PropertySignature;
        if let Some(declaration) = declaration {
            let declaration_store = self
                .type_checker
                .try_source_file_for_node_public(*declaration)
                .map(|source_file| source_file.store())
                .expect("missing member declaration should belong to a checker source file");
            kind = declaration_store.kind(*declaration);
        }
        let declaration_store = declaration.and_then(|declaration| {
            self.type_checker
                .try_source_file_for_node_public(*declaration)
                .map(|source_file| source_file.store())
        });
        let declaration_name = create_declaration_name(
            &mut self.change_tracker.node_factory,
            &mut *self.type_checker,
            Some(symbol),
            declaration,
            declaration_store,
        );
        let modifiers = self.create_modifiers(symbol, declaration);

        let mut flags = nodebuilder::FLAGS_NO_TRUNCATION;
        if quote_preference == lsutil::QuotePreference::Single {
            flags |= nodebuilder::FLAGS_USE_SINGLE_QUOTES_FOR_STRING_LITERAL_TYPE;
        }

        let t = {
            let checker = self.type_checker_mut();
            let symbol_type = checker
                .get_type_of_symbol_identity_at_location_public(
                    symbol,
                    Some(*enclosing_declaration),
                )
                .unwrap_or_else(|| checker.get_error_type());
            checker.get_widened_type_public(symbol_type)
        };
        let mut nodes = Vec::new();

        match kind {
            ast::Kind::PropertySignature | ast::Kind::PropertyDeclaration => {
                let type_node = self.create_type_node(t, enclosing_declaration, flags)?;
                let mut import_state = ast::AstImportState::new();
                let property_name = create_property_name(
                    &mut self.change_tracker.node_factory,
                    &mut import_state,
                    declaration_name.as_ref(),
                    declaration_store,
                    quote_preference,
                )
                .unwrap_or_else(|| self.change_tracker.node_factory.new_identifier(""));
                let mut question_token = None;
                if optional && preserve_optional & PRESERVE_OPTIONAL_FLAGS_PROPERTY != 0 {
                    question_token = Some(
                        self.change_tracker
                            .node_factory
                            .new_token(ast::Kind::QuestionToken),
                    );
                }
                nodes.push(self.change_tracker.node_factory.new_property_declaration(
                    modifiers.clone(),
                    property_name,
                    question_token,
                    type_node,
                    None, /*initializer*/
                ));
                return Ok(nodes);
            }

            ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
                let accessors = ast::get_all_accessor_declarations(
                    declaration_store
                        .as_ref()
                        .expect("accessor declaration should have a source arena"),
                    &declarations,
                    *declaration.expect("accessor declaration should exist"),
                );
                let mut ordered_accessors = Vec::new();
                if accessors.second_accessor.is_none() {
                    ordered_accessors.push(accessors.first_accessor);
                } else {
                    ordered_accessors.push(accessors.first_accessor);
                    ordered_accessors.push(accessors.second_accessor.unwrap());
                }

                for accessor in ordered_accessors {
                    if ast::is_get_accessor_declaration(
                        declaration_store.expect("accessor declaration should have a source arena"),
                        accessor,
                    ) {
                        let accessor_body =
                            self.create_body(source_file.store(), body, ambient, quote_preference);
                        let accessor_type =
                            self.create_type_node(t, enclosing_declaration, flags)?;
                        let mut import_state = ast::AstImportState::new();
                        let property_name = create_property_name(
                            &mut self.change_tracker.node_factory,
                            &mut import_state,
                            declaration_name.as_ref(),
                            declaration_store,
                            quote_preference,
                        )
                        .unwrap_or_else(|| self.change_tracker.node_factory.new_identifier(""));
                        let parameters =
                            synthetic_node_list(&mut self.change_tracker.node_factory, Vec::new());
                        nodes.push(
                            self.change_tracker
                                .node_factory
                                .new_get_accessor_declaration(
                                    modifiers.clone(),
                                    property_name,
                                    None, /*typeParameters*/
                                    parameters,
                                    accessor_type,
                                    None, /*fullSignature*/
                                    accessor_body,
                                ),
                        );
                    }

                    if ast::is_set_accessor_declaration(
                        declaration_store.expect("accessor declaration should have a source arena"),
                        accessor,
                    ) {
                        let parameter = checker::get_set_accessor_value_parameter(
                            declaration_store
                                .as_ref()
                                .expect("accessor declaration should have a source arena"),
                            accessor,
                        );
                        if parameter.is_none() {
                            panic!("Expected set accessor to have a parameter.");
                        }
                        let parameter_name = declaration_store
                            .as_ref()
                            .expect("accessor declaration should have a source arena")
                            .name(parameter.unwrap())
                            .map(|name| {
                                declaration_store
                                    .as_ref()
                                    .expect("accessor declaration should have a source arena")
                                    .text(name)
                            })
                            .unwrap_or_default();
                        let parameter_type =
                            self.create_type_node(t, enclosing_declaration, flags)?;

                        let accessor_body =
                            self.create_body(source_file.store(), body, ambient, quote_preference);
                        let parameters = create_dummy_parameters(
                            &mut self.change_tracker.node_factory,
                            1,
                            &[parameter_name],
                            parameter_type
                                .as_ref()
                                .map(std::slice::from_ref)
                                .unwrap_or(&[]),
                            1,
                            ast::is_in_js_file(source_file.store(), *enclosing_declaration),
                        );
                        let mut import_state = ast::AstImportState::new();
                        let property_name = create_property_name(
                            &mut self.change_tracker.node_factory,
                            &mut import_state,
                            declaration_name.as_ref(),
                            declaration_store,
                            quote_preference,
                        )
                        .unwrap_or_else(|| self.change_tracker.node_factory.new_identifier(""));
                        nodes.push(
                            self.change_tracker
                                .node_factory
                                .new_set_accessor_declaration(
                                    modifiers.clone(),
                                    property_name,
                                    None, /*typeParameters*/
                                    parameters,
                                    None, /*type*/
                                    None, /*fullSignature*/
                                    accessor_body,
                                ),
                        );
                    }
                }
                return Ok(nodes);
            }

            ast::Kind::MethodSignature | ast::Kind::MethodDeclaration => {
                let mut nodes: Vec<ast::Node> = Vec::new();
                let signatures = self.get_call_signatures(t);
                let preserve_optional =
                    optional && preserve_optional & PRESERVE_OPTIONAL_FLAGS_METHOD != 0;
                if signatures.is_empty() {
                    return Ok(Vec::new());
                }

                if declarations.len() == 1 {
                    let method_body =
                        self.create_body(source_file.store(), body, ambient, quote_preference);
                    let method = self.create_signature_declaration_from_signature(
                        signatures[0],
                        ast::Kind::MethodDeclaration,
                        source_file,
                        enclosing_declaration,
                        method_body,
                        modifiers.clone(),
                        declaration_name.clone(),
                        declaration_store,
                        preserve_optional,
                    );
                    if let Some(method) = method {
                        nodes.push(method);
                    }
                    return Ok(nodes);
                }

                for signature in &signatures {
                    if self
                        .type_checker
                        .signature_declaration_public(*signature)
                        .is_some_and(|declaration| {
                            self.type_checker
                                .try_source_file_for_node_public(declaration)
                                .is_some_and(|source_file| {
                                    let store = source_file.store();
                                    store.flags(declaration) & ast::NODE_FLAGS_AMBIENT
                                        != ast::NODE_FLAGS_NONE
                                })
                        })
                    {
                        continue;
                    }

                    let method = self.create_signature_declaration_from_signature(
                        *signature,
                        ast::Kind::MethodDeclaration,
                        source_file,
                        enclosing_declaration,
                        None,
                        modifiers.clone(),
                        declaration_name.clone(),
                        declaration_store,
                        preserve_optional,
                    );
                    if let Some(method) = method {
                        nodes.push(method);
                    }
                }

                if ambient {
                    return Ok(nodes);
                }

                if declarations.len() > signatures.len() {
                    let signature = self
                        .type_checker_mut()
                        .get_signature_from_declaration_public(*declarations.last().unwrap());
                    let method_body =
                        self.create_body(source_file.store(), body, ambient, quote_preference);
                    let method = self.create_signature_declaration_from_signature(
                        signature,
                        ast::Kind::MethodDeclaration,
                        source_file,
                        enclosing_declaration,
                        method_body,
                        modifiers.clone(),
                        declaration_name.clone(),
                        declaration_store,
                        preserve_optional,
                    );
                    if let Some(method) = method {
                        nodes.push(method);
                    }
                } else {
                    let method = self.create_signature_declaration_from_signatures(
                        &signatures,
                        declaration_name.clone(),
                        preserve_optional,
                        modifiers.clone(),
                        quote_preference,
                        body,
                        source_file.store(),
                        declaration_store,
                        enclosing_declaration,
                    )?;
                    if let Some(method) = method {
                        nodes.push(method);
                    }
                }

                return Ok(nodes);
            }
            _ => {}
        }
        Ok(Vec::new())
    }

    fn get_call_signatures(&mut self, t: checker::TypeHandle) -> Vec<checker::SignatureHandle> {
        if self.type_checker.is_union_type_public(t) {
            return self
                .type_checker
                .type_types_public(t)
                .iter()
                .flat_map(|t| self.type_checker_mut().get_call_signatures(*t))
                .collect();
        }
        self.type_checker_mut().get_call_signatures(t)
    }

    fn create_type_node(
        &mut self,
        t: checker::TypeHandle,
        enclosing_declaration: &ast::Node,
        flags: nodebuilder::Flags,
    ) -> Result<Option<ast::Node>, core::Error> {
        let (type_node, id_to_symbol) = self.type_checker.type_to_type_node_for_ls_public(
            &mut self.change_tracker.emit_context,
            t,
            Some(*enclosing_declaration),
            flags,
            nodebuilder::INTERNAL_FLAGS_NONE,
        );
        self.import_type_node(type_node.as_ref(), &id_to_symbol)
    }

    fn create_modifiers(
        &mut self,
        symbol: ast::SymbolIdentity,
        declaration: Option<&ast::Node>,
    ) -> Option<ast::ModifierList> {
        let mut modifier_flags = ast::MODIFIER_FLAGS_NONE;
        if let Some(declaration) = declaration {
            let declaration_store = self
                .type_checker
                .try_source_file_for_node_public(*declaration)
                .map(|source_file| source_file.store())
                .expect("missing member declaration should belong to a checker source file");
            let effective =
                get_declaration_modifier_flags_from_symbol_identity(self.type_checker, symbol);
            modifier_flags = effective & ast::MODIFIER_FLAGS_STATIC;
            if effective & ast::MODIFIER_FLAGS_PUBLIC != ast::MODIFIER_FLAGS_NONE {
                modifier_flags |= ast::MODIFIER_FLAGS_PUBLIC;
            } else if effective & ast::MODIFIER_FLAGS_PROTECTED != ast::MODIFIER_FLAGS_NONE {
                modifier_flags |= ast::MODIFIER_FLAGS_PROTECTED;
            }
            if ast::is_auto_accessor_property_declaration(declaration_store, *declaration) {
                modifier_flags |= ast::MODIFIER_FLAGS_ACCESSOR;
            }
        }
        if self.should_add_override_keyword(declaration) {
            modifier_flags |= ast::MODIFIER_FLAGS_OVERRIDE;
        }
        if modifier_flags == ast::MODIFIER_FLAGS_NONE {
            return None;
        }
        let modifiers = create_modifiers_from_modifier_flags(
            &mut self.change_tracker.node_factory,
            modifier_flags,
        );
        Some(synthetic_modifier_list(
            &mut self.change_tracker.node_factory,
            modifiers,
            modifier_flags,
        ))
    }

    fn should_add_override_keyword(&self, declaration: Option<&ast::Node>) -> bool {
        let Some(declaration) = declaration else {
            return false;
        };
        if !self.program.options().no_implicit_override.is_true() {
            return false;
        }
        let store = self
            .type_checker
            .try_source_file_for_node_public(*declaration)
            .map(|source_file| source_file.store())
            .expect("missing member declaration should belong to a checker source file");
        ast::has_abstract_modifier(store, declaration)
    }

    fn create_signature_declaration_from_signature(
        &mut self,
        signature: checker::SignatureHandle,
        kind: ast::Kind,
        source_file: &'a ast::SourceFile,
        enclosing_declaration: &ast::Node,
        body: Option<ast::Node>,
        modifiers: Option<ast::ModifierList>,
        name: Option<ast::Node>,
        name_store: Option<&ast::AstStore>,
        optional: bool,
    ) -> Option<ast::Node> {
        let quote_preference = lsutil::get_quote_preference(source_file, &self.preferences);
        let mut flags = nodebuilder::FLAGS_NO_TRUNCATION
            | nodebuilder::FLAGS_SUPPRESS_ANY_RETURN_TYPE
            | nodebuilder::FLAGS_ALLOW_EMPTY_TUPLE;
        if quote_preference == lsutil::QuotePreference::Single {
            flags |= nodebuilder::FLAGS_USE_SINGLE_QUOTES_FOR_STRING_LITERAL_TYPE;
        }

        let signature_declaration = self
            .type_checker
            .signature_to_signature_declaration_for_ls_public(
                &mut self.change_tracker.emit_context,
                signature,
                kind,
                Some(*enclosing_declaration),
                flags,
                nodebuilder::INTERNAL_FLAGS_ALLOW_UNRESOLVED_NAMES,
            );
        let Some(signature_declaration) = signature_declaration else {
            return None;
        };

        let is_js = ast::is_in_js_file(source_file.store(), *enclosing_declaration);
        let signature_store = self
            .change_tracker
            .emit_context
            .factory
            .node_factory
            .store();
        let mut import_state = ast::AstImportState::new();
        let source_parameters = signature_store.source_parameters(signature_declaration);
        let source_type_parameters = if is_js {
            None
        } else {
            signature_store.source_type_parameters(signature_declaration)
        };
        let mut parameters: Option<ast::NodeList> = None;
        let mut type_parameters: Option<ast::NodeList> = None;
        let type_node = if is_js {
            None
        } else {
            optional_output_node(
                &mut import_state,
                signature_store,
                &mut self.change_tracker.node_factory,
                signature_store.r#type(signature_declaration),
            )
        };

        if let Some(old_type_parameters) = source_type_parameters {
            let mut nodes = Vec::with_capacity(old_type_parameters.len());
            for tp in old_type_parameters.iter() {
                if ast::is_type_parameter_declaration(signature_store, tp) {
                    let constraint = signature_store.constraint(tp);
                    let default_type = signature_store.default_type(tp);

                    let type_parameter_name = signature_store
                        .name(tp)
                        .map(|name| {
                            output_node(
                                &mut import_state,
                                signature_store,
                                &mut self.change_tracker.node_factory,
                                name,
                            )
                        })
                        .unwrap_or_else(|| self.change_tracker.node_factory.new_identifier(""));
                    let modifiers = optional_output_modifiers(
                        &mut import_state,
                        signature_store,
                        &mut self.change_tracker.node_factory,
                        tp,
                    );
                    let constraint = optional_output_node(
                        &mut import_state,
                        signature_store,
                        &mut self.change_tracker.node_factory,
                        constraint,
                    );
                    let expression = optional_output_node(
                        &mut import_state,
                        signature_store,
                        &mut self.change_tracker.node_factory,
                        signature_store.expression(tp),
                    );
                    let default_type = optional_output_node(
                        &mut import_state,
                        signature_store,
                        &mut self.change_tracker.node_factory,
                        default_type,
                    );
                    nodes.push(
                        self.change_tracker
                            .node_factory
                            .update_type_parameter_declaration_from_store(
                                signature_store,
                                tp,
                                modifiers,
                                type_parameter_name,
                                constraint,
                                expression,
                                default_type,
                            ),
                    );
                } else {
                    nodes.push(output_node(
                        &mut import_state,
                        signature_store,
                        &mut self.change_tracker.node_factory,
                        tp,
                    ));
                }
            }
            type_parameters = Some(output_node_list_from_source(
                &mut import_state,
                &mut self.change_tracker.node_factory,
                old_type_parameters,
                nodes,
            ));
        }

        if let Some(parameter_list) = source_parameters {
            let mut nodes = Vec::with_capacity(parameter_list.len());
            for p in parameter_list.iter() {
                let parameter_type_node = optional_output_node(
                    &mut import_state,
                    signature_store,
                    &mut self.change_tracker.node_factory,
                    signature_store.r#type(p),
                );

                let parameter_name = signature_store
                    .name(p)
                    .map(|name| {
                        output_node(
                            &mut import_state,
                            signature_store,
                            &mut self.change_tracker.node_factory,
                            name,
                        )
                    })
                    .unwrap_or_else(|| self.change_tracker.node_factory.new_identifier(""));
                let modifiers = optional_output_modifiers(
                    &mut import_state,
                    signature_store,
                    &mut self.change_tracker.node_factory,
                    p,
                );
                let dot_dot_dot_token = optional_output_node(
                    &mut import_state,
                    signature_store,
                    &mut self.change_tracker.node_factory,
                    signature_store.dot_dot_dot_token(p),
                );
                let question_token = if is_js {
                    None
                } else {
                    optional_output_node(
                        &mut import_state,
                        signature_store,
                        &mut self.change_tracker.node_factory,
                        signature_store.question_token(p),
                    )
                };
                let initializer = optional_output_node(
                    &mut import_state,
                    signature_store,
                    &mut self.change_tracker.node_factory,
                    signature_store.initializer(p),
                );
                nodes.push(
                    self.change_tracker
                        .node_factory
                        .update_parameter_declaration_from_store(
                            signature_store,
                            p,
                            modifiers,
                            dot_dot_dot_token,
                            parameter_name,
                            question_token,
                            parameter_type_node,
                            initializer,
                        ),
                );
            }
            parameters = Some(output_node_list_from_source(
                &mut import_state,
                &mut self.change_tracker.node_factory,
                parameter_list,
                nodes,
            ));
        }

        let mut question_token = None;
        if optional {
            question_token = Some(
                self.change_tracker
                    .node_factory
                    .new_token(ast::Kind::QuestionToken),
            );
        }
        let asterisk_token = optional_output_node(
            &mut import_state,
            signature_store,
            &mut self.change_tracker.node_factory,
            signature_store.asterisk_token(signature_declaration),
        );
        let full_signature = optional_output_node(
            &mut import_state,
            signature_store,
            &mut self.change_tracker.node_factory,
            signature_store.full_signature(signature_declaration),
        );
        let equals_greater_than_token = optional_output_node(
            &mut import_state,
            signature_store,
            &mut self.change_tracker.node_factory,
            signature_store.equals_greater_than_token(signature_declaration),
        );
        let signature_body = optional_output_node(
            &mut import_state,
            signature_store,
            &mut self.change_tracker.node_factory,
            signature_store.body(signature_declaration),
        );

        match kind {
            ast::Kind::FunctionExpression => {
                let body_node = body
                    .or(signature_body)
                    .unwrap_or_else(|| self.create_stubbed_method_body(quote_preference));
                let parameters = parameters.clone().unwrap_or_else(|| {
                    synthetic_node_list(&mut self.change_tracker.node_factory, Vec::new())
                });
                let name = name.as_ref().and_then(|name| {
                    optional_identifier_output_node(
                        &mut import_state,
                        &mut self.change_tracker.node_factory,
                        Some(name),
                        name_store,
                    )
                });
                Some(self.change_tracker.node_factory.new_function_expression(
                    modifiers.clone(),
                    asterisk_token,
                    name,
                    type_parameters.clone(),
                    parameters,
                    type_node.clone(),
                    full_signature,
                    body_node,
                ))
            }
            ast::Kind::ArrowFunction => {
                let body_node = body
                    .or(signature_body)
                    .unwrap_or_else(|| self.create_stubbed_method_body(quote_preference));
                let parameters = parameters.clone().unwrap_or_else(|| {
                    synthetic_node_list(&mut self.change_tracker.node_factory, Vec::new())
                });
                Some(self.change_tracker.node_factory.new_arrow_function(
                    modifiers.clone(),
                    type_parameters.clone(),
                    parameters,
                    type_node.clone(),
                    full_signature,
                    equals_greater_than_token,
                    body_node,
                ))
            }
            ast::Kind::MethodDeclaration => {
                let method_name = if name.is_none() {
                    self.change_tracker.node_factory.new_identifier("")
                } else {
                    create_property_name(
                        &mut self.change_tracker.node_factory,
                        &mut import_state,
                        name.as_ref(),
                        name_store,
                        quote_preference,
                    )
                    .unwrap_or_else(|| self.change_tracker.node_factory.new_identifier(""))
                };
                let parameters = parameters.clone().unwrap_or_else(|| {
                    synthetic_node_list(&mut self.change_tracker.node_factory, Vec::new())
                });
                Some(self.change_tracker.node_factory.new_method_declaration(
                    modifiers.clone(),
                    asterisk_token,
                    method_name,
                    question_token,
                    type_parameters.clone(),
                    parameters,
                    type_node.clone(),
                    full_signature,
                    body,
                ))
            }
            ast::Kind::FunctionDeclaration => {
                let parameters = parameters.unwrap_or_else(|| {
                    synthetic_node_list(&mut self.change_tracker.node_factory, Vec::new())
                });
                let name = name.as_ref().and_then(|name| {
                    optional_identifier_output_node(
                        &mut import_state,
                        &mut self.change_tracker.node_factory,
                        Some(name),
                        name_store,
                    )
                });
                Some(self.change_tracker.node_factory.new_function_declaration(
                    modifiers.clone(),
                    asterisk_token,
                    name,
                    type_parameters.clone(),
                    parameters,
                    type_node,
                    full_signature,
                    body.or(signature_body),
                ))
            }
            _ => None,
        }
    }

    fn create_signature_declaration_from_signatures(
        &mut self,
        signatures: &[checker::SignatureHandle],
        name: Option<ast::Node>,
        optional: bool,
        modifiers: Option<ast::ModifierList>,
        quote_preference: lsutil::QuotePreference,
        body: Option<&ast::Node>,
        body_source: &ast::AstStore,
        name_store: Option<&ast::AstStore>,
        enclosing_declaration: &ast::Node,
    ) -> Result<Option<ast::Node>, core::Error> {
        if signatures.is_empty() {
            return Ok(None);
        }

        let mut max_args_signature = signatures[0];
        let mut min_argument_count =
            self.type_checker
                .signature_min_argument_count_public(signatures[0]) as usize;

        let mut has_rest_parameter = false;
        for signature in signatures {
            min_argument_count = min_argument_count.min(
                self.type_checker
                    .signature_min_argument_count_public(*signature) as usize,
            );
            if self
                .type_checker
                .signature_has_rest_parameter_public(*signature)
            {
                has_rest_parameter = true;
            }
            if self
                .type_checker
                .signature_parameters_public(*signature)
                .len()
                >= self
                    .type_checker
                    .signature_parameters_public(max_args_signature)
                    .len()
                && (!self
                    .type_checker
                    .signature_has_rest_parameter_public(*signature)
                    || self
                        .type_checker
                        .signature_has_rest_parameter_public(max_args_signature))
            {
                max_args_signature = *signature;
            }
        }

        let max_non_rest_args = self
            .type_checker
            .signature_parameters_public(max_args_signature)
            .len()
            - if self
                .type_checker
                .signature_has_rest_parameter_public(max_args_signature)
            {
                1
            } else {
                0
            };
        let parameter_symbols = self
            .type_checker
            .signature_parameters_public(max_args_signature);
        let parameter_names = parameter_symbols
            .iter()
            .map(|symbol| {
                self.type_checker
                    .symbol_name_public(*symbol)
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let mut parameter_nodes = create_dummy_parameter_nodes(
            &mut self.change_tracker.node_factory,
            max_non_rest_args,
            &parameter_names,
            &[],
            min_argument_count,
            ast::is_in_js_file(body_source, *enclosing_declaration),
        );

        if has_rest_parameter {
            let mut rest_parameter_name = "rest".to_string();
            if max_non_rest_args < parameter_names.len()
                && !parameter_names[max_non_rest_args].is_empty()
            {
                rest_parameter_name = parameter_names[max_non_rest_args].clone();
            }

            let mut question_token = None;
            if max_non_rest_args >= min_argument_count {
                question_token = Some(
                    self.change_tracker
                        .node_factory
                        .new_token(ast::Kind::QuestionToken),
                );
            }

            let dot_dot_dot_token = self
                .change_tracker
                .node_factory
                .new_token(ast::Kind::DotDotDotToken);
            let rest_name = self
                .change_tracker
                .node_factory
                .new_identifier(&rest_parameter_name);
            let unknown_type = self
                .change_tracker
                .node_factory
                .new_keyword_type_node(ast::Kind::UnknownKeyword);
            let rest_type = self
                .change_tracker
                .node_factory
                .new_array_type_node(unknown_type);
            let rest_parameter = self.change_tracker.node_factory.new_parameter_declaration(
                None, /*modifiers*/
                Some(dot_dot_dot_token),
                rest_name,
                question_token,
                Some(rest_type),
                None, /*initializer*/
            );
            parameter_nodes.push(rest_parameter);
        }
        let parameters =
            synthetic_node_list(&mut self.change_tracker.node_factory, parameter_nodes);

        let method_name = if name.is_none() {
            self.change_tracker.node_factory.new_identifier("")
        } else {
            let mut import_state = ast::AstImportState::new();
            create_property_name(
                &mut self.change_tracker.node_factory,
                &mut import_state,
                name.as_ref(),
                name_store,
                quote_preference,
            )
            .unwrap_or_else(|| self.change_tracker.node_factory.new_identifier(""))
        };

        let return_type =
            self.get_return_type_from_signatures(signatures, enclosing_declaration)?;
        let method_body =
            self.create_body(body_source, body, false /*ambient*/, quote_preference);
        let optional_token = if optional {
            Some(
                self.change_tracker
                    .node_factory
                    .new_token(ast::Kind::QuestionToken),
            )
        } else {
            None
        };
        Ok(Some(
            self.change_tracker.node_factory.new_method_declaration(
                modifiers.clone(),
                None, /*asteriskToken*/
                method_name,
                optional_token,
                None, /*typeParameters*/
                parameters,
                return_type,
                None, /*fullSignature*/
                method_body,
            ),
        ))
    }

    fn get_return_type_from_signatures(
        &mut self,
        signatures: &[checker::SignatureHandle],
        enclosing_declaration: &ast::Node,
    ) -> Result<Option<ast::Node>, core::Error> {
        if signatures.is_empty() {
            return Ok(None);
        }

        let return_types = signatures
            .iter()
            .map(|signature| {
                self.type_checker_mut()
                    .get_return_type_of_signature_public(*signature)
            })
            .collect::<Vec<_>>();

        let union_type = self.type_checker_mut().get_union_type_public(return_types);
        let (type_node, id_to_symbol) = self.type_checker.type_to_type_node_for_ls_public(
            &mut self.change_tracker.emit_context,
            union_type,
            Some(*enclosing_declaration),
            nodebuilder::FLAGS_NO_TRUNCATION,
            nodebuilder::INTERNAL_FLAGS_ALLOW_UNRESOLVED_NAMES,
        );
        self.import_type_node(type_node.as_ref(), &id_to_symbol)
    }

    fn import_type_node(
        &mut self,
        type_node: Option<&ast::Node>,
        id_to_symbol: &HashMap<ast::Node, ast::SymbolIdentity>,
    ) -> Result<Option<ast::Node>, core::Error> {
        let Some(type_node) = type_node else {
            return Ok(None);
        };
        let named_id_to_symbol = id_to_symbol
            .iter()
            .filter_map(|(identifier, symbol)| {
                let name = self.type_checker.symbol_name_public(*symbol)?;
                Some((*identifier, (*symbol, name)))
            })
            .collect::<HashMap<_, _>>();
        let result = {
            let change_tracker = &mut *self.change_tracker;
            let source = change_tracker.emit_context.factory.node_factory.store();
            let factory = &mut change_tracker.node_factory;
            autoimport::try_get_auto_importable_reference_from_type_node(
                source,
                factory,
                type_node,
                named_id_to_symbol,
            )
        };
        let Some(import_adder) = self.import_adder.as_deref_mut() else {
            return Ok(Some(result.type_node));
        };

        if result.converted {
            let checker = &mut *self.type_checker;
            for symbol in result.symbols {
                import_adder.add_import_from_exported_symbol(
                    checker, symbol, true, /*isValidTypeOnlyUseSite*/
                )?;
            }
            return Ok(Some(result.type_node));
        }

        let mut seen: HashMap<ast::SymbolIdentity, bool> = HashMap::new();
        for symbol in id_to_symbol.values() {
            let symbol_key = *symbol;
            if seen.get(&symbol_key).copied().unwrap_or(false) {
                continue;
            }
            seen.insert(symbol_key, true);
            let checker = &mut *self.type_checker;
            import_adder.add_import_from_exported_symbol(
                checker, *symbol, true, /*isValidTypeOnlyUseSite*/
            )?;
        }
        Ok(Some(result.type_node))
    }

    pub fn create_index_signature_declaration_from_type(
        &mut self,
        class_declaration: &ast::Node,
        implemented_type: checker::TypeHandle,
        key_type: checker::TypeHandle,
    ) -> Option<ast::Node> {
        let index_info = self
            .type_checker_mut()
            .get_index_info_of_type_public(implemented_type, key_type);
        let Some(index_info) = index_info else {
            return None;
        };

        let node = self
            .type_checker
            .index_info_to_index_signature_declaration_for_ls_public(
                &mut self.change_tracker.emit_context,
                index_info,
                Some(*class_declaration),
                nodebuilder::FLAGS_NONE,
                nodebuilder::INTERNAL_FLAGS_NONE,
            )?;
        let signature_store = self
            .change_tracker
            .emit_context
            .factory
            .node_factory
            .store();
        let mut import_state = ast::AstImportState::new();
        Some(output_node(
            &mut import_state,
            signature_store,
            &mut self.change_tracker.node_factory,
            node,
        ))
    }

    fn create_body(
        &mut self,
        source: &ast::AstStore,
        body: Option<&ast::Node>,
        ambient: bool,
        quote_preference: lsutil::QuotePreference,
    ) -> Option<ast::Node> {
        if ambient {
            return None;
        }
        let body = body.map(|body| {
            self.change_tracker
                .node_factory
                .deep_clone_node_from_store(source, *body)
        });
        Some(body.unwrap_or_else(|| self.create_stubbed_method_body(quote_preference)))
    }

    fn create_stubbed_method_body(
        &mut self,
        quote_preference: lsutil::QuotePreference,
    ) -> ast::Node {
        let mut token_flags = ast::TOKEN_FLAGS_NONE;
        if quote_preference == lsutil::QuotePreference::Single {
            token_flags = ast::TOKEN_FLAGS_SINGLE_QUOTE;
        }

        let factory = &mut self.change_tracker.node_factory;
        let message = factory.new_string_literal(
            diagnostics::METHOD_NOT_IMPLEMENTED.localize(self.locale.clone(), vec![]),
            token_flags,
        );
        let arguments = synthetic_node_list(factory, vec![message]);
        let error = factory.new_identifier("Error");
        let new_error = factory.new_new_expression(error, None, Some(arguments));
        let throw_statement = factory.new_throw_statement(new_error);
        let statements = synthetic_node_list(factory, vec![throw_statement]);
        factory.new_block(statements, true /*multiLine*/)
    }
}

fn create_dummy_parameter_nodes(
    factory: &mut ast::NodeFactory,
    arg_count: usize,
    names: &[String],
    types: &[ast::TypeNode],
    min_argument_count: usize,
    in_js: bool,
) -> Vec<ast::Node> {
    let mut parameters = Vec::with_capacity(arg_count);
    let mut parameter_name_counts: HashMap<String, usize> = HashMap::new();

    for i in 0..arg_count {
        let mut parameter_name = if i < names.len() && !names[i].is_empty() {
            names[i].clone()
        } else {
            "arg".to_string() + &i.to_string()
        };

        let count = parameter_name_counts
            .get(&parameter_name)
            .copied()
            .unwrap_or(0);
        parameter_name_counts.insert(parameter_name.clone(), count + 1);

        if count > 0 {
            parameter_name.push_str(&count.to_string());
        }

        let mut question_token = None;
        if i >= min_argument_count {
            question_token = Some(factory.new_token(ast::Kind::QuestionToken));
        }

        let type_node = if in_js {
            None
        } else if i < types.len() {
            Some(types[i].clone())
        } else {
            Some(factory.new_keyword_type_node(ast::Kind::UnknownKeyword))
        };
        let parameter_name_node = factory.new_identifier(&parameter_name);
        parameters.push(factory.new_parameter_declaration(
            None, /*modifiers*/
            None, /*dotDotDotToken*/
            parameter_name_node,
            question_token,
            type_node,
            None, /*initializer*/
        ));
    }
    parameters
}

fn create_dummy_parameters(
    factory: &mut ast::NodeFactory,
    arg_count: usize,
    names: &[String],
    types: &[ast::TypeNode],
    min_argument_count: usize,
    in_js: bool,
) -> ast::NodeList {
    let parameters =
        create_dummy_parameter_nodes(factory, arg_count, names, types, min_argument_count, in_js);
    synthetic_node_list(factory, parameters)
}

fn create_modifiers_from_modifier_flags(
    factory: &mut ast::NodeFactory,
    flags: ast::ModifierFlags,
) -> Vec<ast::Node> {
    let mut result = Vec::new();
    if flags & ast::MODIFIER_FLAGS_EXPORT != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::ExportKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_AMBIENT != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::DeclareKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_DEFAULT != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::DefaultKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_PUBLIC != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::PublicKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_PROTECTED != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::ProtectedKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_PRIVATE != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::PrivateKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_STATIC != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::StaticKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_OVERRIDE != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::OverrideKeyword));
    }
    if flags & ast::MODIFIER_FLAGS_ACCESSOR != ast::MODIFIER_FLAGS_NONE {
        result.push(factory.new_modifier(ast::Kind::AccessorKeyword));
    }
    result
}

fn create_declaration_name<'a>(
    factory: &mut ast::NodeFactory,
    type_checker: &mut checker::Checker<'a, '_>,
    symbol: Option<ast::SymbolIdentity>,
    declaration: Option<&ast::Node>,
    declaration_store: Option<&ast::AstStore>,
) -> Option<ast::Node> {
    if symbol.is_some_and(|symbol| {
        type_checker
            .symbol_check_flags_public(symbol)
            .is_some_and(|flags| flags & ast::CHECK_FLAGS_MAPPED != ast::CHECK_FLAGS_NONE)
    }) {
        let symbol = symbol.unwrap();
        let name_type = type_checker.get_name_type_of_symbol_identity_public(symbol);
        if let Some(name_type) = name_type.filter(|name_type| {
            checker::is_type_usable_as_property_name_public(type_checker, *name_type)
        }) {
            return Some(
                factory.new_identifier(&checker::get_property_name_from_type_public(
                    type_checker,
                    name_type,
                )),
            );
        }
    }
    if let Some(declaration) = declaration {
        let store = declaration_store.expect("declaration name should have a source arena");
        if store.name(*declaration).is_some() {
            return store.name(*declaration);
        }
    }
    if let Some(symbol) = symbol {
        return Some(
            factory.new_identifier(&type_checker.symbol_name_public(symbol).unwrap_or_default()),
        );
    }
    None
}

fn create_property_name(
    factory: &mut ast::NodeFactory,
    import_state: &mut ast::AstImportState,
    node: Option<&ast::Node>,
    source_store: Option<&ast::AstStore>,
    quote_preference: lsutil::QuotePreference,
) -> Option<ast::Node> {
    let Some(node) = node else {
        return None;
    };
    if node.store_id() == factory.store().store_id() {
        let is_constructor = {
            let node_store = factory.store();
            ast::is_identifier(node_store, *node) && node_store.text(*node) == "constructor"
        };
        if is_constructor {
            let mut token_flags = ast::TOKEN_FLAGS_NONE;
            if quote_preference == lsutil::QuotePreference::Single {
                token_flags = ast::TOKEN_FLAGS_SINGLE_QUOTE;
            }
            let literal = factory.new_string_literal("constructor", token_flags);
            return Some(factory.new_computed_property_name(literal));
        }
        return Some(*node);
    }

    let node_store = source_store.expect("property name should have a source arena");
    assert_eq!(
        node.store_id(),
        node_store.store_id(),
        "property name belongs to an unexpected AST store"
    );
    let node_text = node_store.text(*node);
    if ast::is_identifier(node_store, *node) && node_text == "constructor" {
        let mut token_flags = ast::TOKEN_FLAGS_NONE;
        if quote_preference == lsutil::QuotePreference::Single {
            token_flags = ast::TOKEN_FLAGS_SINGLE_QUOTE;
        }
        let literal = factory.new_string_literal(&node_text, token_flags);
        return Some(factory.new_computed_property_name(literal));
    }
    Some(output_node(import_state, node_store, factory, *node))
}
