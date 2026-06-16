use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeEraserAction {
    Keep,
    Elide,
    StripTypeSyntax,
}

pub fn type_eraser_action_for_kind(kind: ast::Kind) -> TypeEraserAction {
    match kind {
        // TypeScript accessibility and readonly modifiers are elided
        ast::Kind::PublicKeyword
        | ast::Kind::PrivateKeyword
        | ast::Kind::ProtectedKeyword
        | ast::Kind::AbstractKeyword
        | ast::Kind::OverrideKeyword
        | ast::Kind::ConstKeyword
        | ast::Kind::DeclareKeyword
        | ast::Kind::ReadonlyKeyword
        // TypeScript type nodes are elided.
        | ast::Kind::ArrayType
        | ast::Kind::TupleType
        | ast::Kind::OptionalType
        | ast::Kind::RestType
        | ast::Kind::TypeLiteral
        | ast::Kind::TypePredicate
        | ast::Kind::TypeParameter
        | ast::Kind::AnyKeyword
        | ast::Kind::UnknownKeyword
        | ast::Kind::BooleanKeyword
        | ast::Kind::StringKeyword
        | ast::Kind::NumberKeyword
        | ast::Kind::NeverKeyword
        | ast::Kind::VoidKeyword
        | ast::Kind::SymbolKeyword
        | ast::Kind::ConstructorType
        | ast::Kind::FunctionType
        | ast::Kind::TypeQuery
        | ast::Kind::TypeReference
        | ast::Kind::UnionType
        | ast::Kind::IntersectionType
        | ast::Kind::ConditionalType
        | ast::Kind::ParenthesizedType
        | ast::Kind::ThisType
        | ast::Kind::TypeOperator
        | ast::Kind::IndexedAccessType
        | ast::Kind::MappedType
        | ast::Kind::LiteralType
        // TypeScript index signatures are elided.
        | ast::Kind::IndexSignature
        | ast::Kind::JSImportDeclaration
        | ast::Kind::NamespaceExportDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::JSTypeAliasDeclaration
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::ImportType => TypeEraserAction::Elide,
        ast::Kind::ExpressionWithTypeArguments
        | ast::Kind::PropertyDeclaration
        | ast::Kind::Constructor
        | ast::Kind::MethodDeclaration
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::VariableDeclaration
        | ast::Kind::HeritageClause
        | ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction
        | ast::Kind::Parameter
        | ast::Kind::CallExpression
        | ast::Kind::NewExpression
        | ast::Kind::TaggedTemplateExpression
        | ast::Kind::NonNullExpression
        | ast::Kind::TypeAssertionExpression
        | ast::Kind::AsExpression
        | ast::Kind::SatisfiesExpression
        | ast::Kind::ParenthesizedExpression
        | ast::Kind::JsxSelfClosingElement
        | ast::Kind::JsxOpeningElement
        | ast::Kind::ImportEqualsDeclaration
        | ast::Kind::ImportDeclaration
        | ast::Kind::ImportClause
        | ast::Kind::NamedImports
        | ast::Kind::ImportSpecifier
        | ast::Kind::ExportDeclaration
        | ast::Kind::NamedExports
        | ast::Kind::ExportSpecifier
        | ast::Kind::ModuleDeclaration
        | ast::Kind::EnumDeclaration => TypeEraserAction::StripTypeSyntax,
        _ => TypeEraserAction::Keep,
    }
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    let mut runtime = TypeEraserRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        compiler_options,
        parent_node: None,
        current_node: None,
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

struct TypeEraserRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    compiler_options: &'ctx core::CompilerOptions,
    parent_node: Option<ast::Node>,
    current_node: Option<ast::Node>,
}

impl TypeEraserRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn visit_each_child(&mut self, node: &ast::Node) -> ast::Node {
        self.generated_visit_each_child(node)
    }

    fn preserve_optional_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.preserve_node(node))
    }

    fn push_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        let grandparent = self.parent_node;
        self.parent_node = self.current_node;
        self.current_node = Some(node);
        grandparent
    }

    fn pop_node(&mut self, grandparent: Option<ast::Node>) {
        self.current_node = self.parent_node;
        self.parent_node = grandparent;
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let factory_store_id = self.factory().store().store_id();
        let original = if node.store_id() == factory_store_id {
            Some(self.emit_context.most_original(node))
        } else {
            assert_eq!(
                (*node).store_id(),
                self.source.store_id(),
                "transform traversal cannot read unrelated AST store"
            );
            None
        };

        let store = self.store_for(*node);
        let kind = store.kind(*node);
        let contains_type_script = store
            .subtree_facts(*node)
            .intersects(ast::SubtreeFacts::CONTAINS_TYPE_SCRIPT)
            || original
                .filter(|original| {
                    *original != *node && original.store_id() == self.source.store_id()
                })
                .is_some_and(|original| {
                    self.source
                        .subtree_facts(original)
                        .intersects(ast::SubtreeFacts::CONTAINS_TYPE_SCRIPT)
                });
        let is_ambient_statement = ast::is_statement(store, *node)
            && ast::has_syntactic_modifier(store, *node, ast::ModifierFlags::AMBIENT);

        if !contains_type_script {
            return Some(*node);
        }
        if is_ambient_statement {
            return Some(self.elide_statement(node));
        }
        let grandparent = self.push_node(*node);
        let result = if node.store_id() == factory_store_id {
            match type_eraser_action_for_kind(kind) {
                TypeEraserAction::Elide => None,
                TypeEraserAction::StripTypeSyntax => self.strip_type_syntax_in_factory_store(*node),
                _ => Some(self.visit_each_child(node)),
            }
        } else {
            self.visit_type_script_node(self.source, *node)
        }
        .map(|result| {
            if result != *node {
                self.emit_context.set_original(&result, node);
            }
            result
        });
        self.pop_node(grandparent);
        result
    }

    fn visit_type_script_node(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
    ) -> Option<ast::Node> {
        match source.kind(node) {
            ast::Kind::PublicKeyword
            | ast::Kind::PrivateKeyword
            | ast::Kind::ProtectedKeyword
            | ast::Kind::AbstractKeyword
            | ast::Kind::OverrideKeyword
            | ast::Kind::ConstKeyword
            | ast::Kind::DeclareKeyword
            | ast::Kind::ReadonlyKeyword
            | ast::Kind::ArrayType
            | ast::Kind::TupleType
            | ast::Kind::OptionalType
            | ast::Kind::RestType
            | ast::Kind::TypeLiteral
            | ast::Kind::TypePredicate
            | ast::Kind::TypeParameter
            | ast::Kind::AnyKeyword
            | ast::Kind::UnknownKeyword
            | ast::Kind::BooleanKeyword
            | ast::Kind::StringKeyword
            | ast::Kind::NumberKeyword
            | ast::Kind::NeverKeyword
            | ast::Kind::VoidKeyword
            | ast::Kind::SymbolKeyword
            | ast::Kind::ConstructorType
            | ast::Kind::FunctionType
            | ast::Kind::TypeQuery
            | ast::Kind::TypeReference
            | ast::Kind::UnionType
            | ast::Kind::IntersectionType
            | ast::Kind::ConditionalType
            | ast::Kind::ParenthesizedType
            | ast::Kind::ThisType
            | ast::Kind::TypeOperator
            | ast::Kind::IndexedAccessType
            | ast::Kind::MappedType
            | ast::Kind::LiteralType
            | ast::Kind::IndexSignature => None,
            ast::Kind::JSImportDeclaration => {
                // reparsed commonjs are elided
                None
            }
            ast::Kind::NamespaceExportDeclaration => {
                // TypeScript namespace export declarations are elided.
                None
            }
            ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::InterfaceDeclaration => {
                // TypeScript type-only declarations are elided.
                Some(self.elide_statement(&node))
            }
            ast::Kind::ModuleDeclaration => self.erase_module_declaration(source, node),
            _ => self.strip_type_syntax(source, node),
        }
    }

    fn elide_statement(&mut self, node: &ast::Node) -> ast::Node {
        self.emit_context.new_not_emitted_statement(node)
    }

    fn erase_module_declaration(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
    ) -> Option<ast::Node> {
        if self.should_elide_module_declaration(source, node) {
            // TypeScript module declarations are elided if they are not instantiated or have no body
            return Some(self.elide_statement(&node));
        }
        Some(self.visit_each_child(&node))
    }

    fn should_elide_module_declaration(&self, source: &ast::AstStore, node: ast::Node) -> bool {
        source
            .name(node)
            .is_none_or(|name| !ast::is_identifier(source, name))
            || !ast::is_instantiated_module(
                source,
                node,
                self.compiler_options.should_preserve_const_enums(),
            )
            || source
                .body(get_innermost_module_declaration_from_dotted_module(
                    source, node,
                ))
                .is_none()
    }

    fn strip_type_syntax(&mut self, source: &ast::AstStore, node: ast::Node) -> Option<ast::Node> {
        match source.kind(node) {
            ast::Kind::ExpressionWithTypeArguments => {
                let expression = self.visit_node(source.expression(node));
                return Some(
                    self.factory_mut()
                        .update_expression_with_type_arguments_from_store(
                            source, node, expression, None,
                        ),
                );
            }
            ast::Kind::Constructor => {
                let body = source.body(node);
                if body.is_none() {
                    // TypeScript overloads are elided
                    return None;
                }
                let parameters = self
                    .visit_parameters_input(
                        (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("constructor parameters are required");
                let body = self.visit_function_body(source.body(node));
                Some(
                    self.factory_mut()
                        .update_constructor_declaration_from_store(
                            source, node, None, None, parameters, None, None, body,
                        ),
                )
            }
            ast::Kind::PropertyDeclaration => {
                let has_ambient_or_abstract = ast::has_syntactic_modifier(
                    source,
                    node,
                    ast::ModifierFlags::AMBIENT | ast::ModifierFlags::ABSTRACT,
                );
                if self.compiler_options.experimental_decorators.is_true()
                    && has_ambient_or_abstract
                    && ast::has_decorators(source, node)
                {
                    // declare/abstract props with decorators must be preserved until the decorator transform can process them and remove them
                } else if has_ambient_or_abstract {
                    // TypeScript `declare` fields are elided
                    return None;
                }
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let initializer = self.visit_node(source.initializer(node));
                let updated = self.factory_mut().update_property_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    name,
                    None,
                    None,
                    initializer,
                );
                Some(updated)
            }
            ast::Kind::MethodDeclaration => {
                let body = source.body(node);
                if ast::node_is_missing(source, body) {
                    // TypeScript overloads are elided
                    return None;
                }
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let parameters = self
                    .visit_nodes_input(
                        (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("method parameters are required");
                let body = self.visit_node(source.body(node));
                let asterisk_token = self.preserve_optional_node(source.asterisk_token(node));
                let updated = self.factory_mut().update_method_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    asterisk_token,
                    name,
                    None,
                    None,
                    parameters,
                    None,
                    None,
                    body,
                );
                Some(updated)
            }
            ast::Kind::GetAccessor => {
                let original_body = source.body(node);
                if ast::node_is_missing(source, original_body)
                    && ast::has_syntactic_modifier(source, node, ast::ModifierFlags::ABSTRACT)
                {
                    // Abstract accessors are elided
                    return None;
                }
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let parameters = self
                    .visit_nodes_input(
                        (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("accessor parameters are required");
                let mut body = self.visit_node(source.body(node));
                if body.is_none() {
                    let empty_statements = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        Vec::<ast::Node>::new(),
                    );
                    body = Some(self.factory_mut().new_block(empty_statements, false));
                }
                let updated = self
                    .factory_mut()
                    .update_get_accessor_declaration_from_store(
                        source, node, modifiers, name, None, parameters, None, None, body,
                    );
                Some(updated)
            }
            ast::Kind::SetAccessor => {
                let original_body = source.body(node);
                if ast::node_is_missing(source, original_body)
                    && ast::has_syntactic_modifier(source, node, ast::ModifierFlags::ABSTRACT)
                {
                    // Abstract accessors are elided
                    return None;
                }
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let parameters = self
                    .visit_nodes_input(
                        (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("accessor parameters are required");
                let mut body = self.visit_node(source.body(node));
                if body.is_none() {
                    let empty_statements = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        Vec::<ast::Node>::new(),
                    );
                    body = Some(self.factory_mut().new_block(empty_statements, false));
                }
                let updated = self
                    .factory_mut()
                    .update_set_accessor_declaration_from_store(
                        source, node, modifiers, name, None, parameters, None, None, body,
                    );
                Some(updated)
            }
            ast::Kind::HeritageClause => {
                let token = source
                    .token(node)
                    .expect("heritage clause should have token");
                if token == ast::Kind::ImplementsKeyword {
                    // TypeScript `implements` clauses are elided
                    return None;
                }
                let types = self
                    .visit_nodes_input(
                        (source.source_types(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("heritage clause types are required");
                Some(
                    self.factory_mut()
                        .update_heritage_clause_from_store(source, node, token, types),
                )
            }
            ast::Kind::VariableDeclaration => {
                let name = self.visit_node(source.name(node));
                let initializer = self.visit_node(source.initializer(node));
                let updated = self.factory_mut().update_variable_declaration_from_store(
                    source,
                    node,
                    name,
                    None,
                    None,
                    initializer,
                );
                if let Some(type_node) = source.r#type(node) {
                    let name = self
                        .factory()
                        .store()
                        .name(updated)
                        .expect("updated variable declaration should have a name");
                    self.emit_context.set_type_node(&name, &type_node);
                }
                Some(updated)
            }
            ast::Kind::Parameter => {
                if ast::is_this_parameter(source, node) {
                    // TypeScript `this` parameters are elided
                    return None;
                }
                // preserve parameter property modifiers to be handled by the runtime transformer
                let modifiers = self
                    .parent_node
                    .filter(|parent| ast::is_parameter_property_declaration(source, node, *parent))
                    .and_then(|_| {
                        self.preserve_parameter_property_modifiers(source.source_modifiers(node))
                    })
                    // preserve decorators for the decorator transforms
                    .or_else(|| self.preserve_parameter_decorators(source.source_modifiers(node)));
                let name = self.visit_node(source.name(node));
                let initializer = self.visit_node(source.initializer(node));
                let dot_dot_dot_token = self.preserve_optional_node(source.dot_dot_dot_token(node));
                let updated = self.factory_mut().update_parameter_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    dot_dot_dot_token,
                    name,
                    None,
                    None,
                    initializer,
                );
                Some(updated)
            }
            ast::Kind::ClassDeclaration => {
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let heritage_clauses = self.visit_nodes_input(
                    (source.source_heritage_clauses(node))
                        .map(ast::SourceNodeListInput::from_source),
                );
                let members = self
                    .visit_nodes_input(
                        (source.source_members(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("class members are required");
                Some(self.factory_mut().update_class_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    name,
                    None,
                    heritage_clauses,
                    members,
                ))
            }
            ast::Kind::ClassExpression => {
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let heritage_clauses = self.visit_nodes_input(
                    (source.source_heritage_clauses(node))
                        .map(ast::SourceNodeListInput::from_source),
                );
                let members = self
                    .visit_nodes_input(
                        (source.source_members(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("class members are required");
                Some(self.factory_mut().update_class_expression_from_store(
                    source,
                    node,
                    modifiers,
                    name,
                    None,
                    heritage_clauses,
                    members,
                ))
            }
            ast::Kind::FunctionDeclaration => {
                let body = source.body(node);
                if ast::node_is_missing(source, body) {
                    // TypeScript overloads are elided
                    return Some(self.elide_statement(&node));
                }
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let parameters = self
                    .visit_nodes_input(
                        (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("function parameters are required");
                let body = self.visit_node(source.body(node));
                let asterisk_token = self.preserve_optional_node(source.asterisk_token(node));
                Some(self.factory_mut().update_function_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    asterisk_token,
                    name,
                    None,
                    parameters,
                    None,
                    None,
                    body,
                ))
            }
            ast::Kind::FunctionExpression => {
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let name = self.visit_node(source.name(node));
                let parameters = self
                    .visit_parameters_input(
                        (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("function parameters are required");
                let body = self.visit_function_body(source.body(node));
                let asterisk_token = self.preserve_optional_node(source.asterisk_token(node));
                let updated = self.factory_mut().update_function_expression_from_store(
                    source,
                    node,
                    modifiers,
                    asterisk_token,
                    name,
                    None,
                    parameters,
                    None,
                    None,
                    body,
                );
                Some(updated)
            }
            ast::Kind::ArrowFunction => {
                let modifiers = self.visit_modifiers_input(
                    (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source),
                );
                let parameters = self
                    .visit_nodes_input(
                        (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("arrow function parameters are required");
                let equals_greater_than_token =
                    self.preserve_optional_node(source.equals_greater_than_token(node));
                let body = self.visit_node(source.body(node));
                Some(self.factory_mut().update_arrow_function_from_store(
                    source,
                    node,
                    modifiers,
                    None,
                    parameters,
                    None,
                    None,
                    equals_greater_than_token,
                    body,
                ))
            }
            ast::Kind::CallExpression => {
                let expression = self.visit_node(source.expression(node));
                let question_dot_token =
                    self.preserve_optional_node(source.question_dot_token(node));
                let arguments = self
                    .visit_nodes_input(
                        (source.source_arguments(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("call expression arguments are required");
                Some(self.factory_mut().update_call_expression_from_store(
                    source,
                    node,
                    expression,
                    question_dot_token,
                    None,
                    arguments,
                    source.flags(node),
                ))
            }
            ast::Kind::NewExpression => {
                let expression = self.visit_node(source.expression(node));
                let arguments = self.visit_nodes_input(
                    (source.source_arguments(node)).map(ast::SourceNodeListInput::from_source),
                );
                Some(
                    self.factory_mut().update_new_expression_from_store(
                        source, node, expression, None, arguments,
                    ),
                )
            }
            ast::Kind::TaggedTemplateExpression => {
                let tag = self.visit_node(source.tag(node));
                let question_dot_token =
                    self.preserve_optional_node(source.question_dot_token(node));
                let template = self.visit_node(source.template(node));
                Some(
                    self.factory_mut()
                        .update_tagged_template_expression_from_store(
                            source,
                            node,
                            tag,
                            question_dot_token,
                            None,
                            template,
                            source.flags(node),
                        ),
                )
            }
            ast::Kind::NonNullExpression
            | ast::Kind::TypeAssertionExpression
            | ast::Kind::AsExpression
            | ast::Kind::SatisfiesExpression => {
                let expression = self.visit_node(source.expression(node));
                Some(self.new_partially_emitted_expression(source, node, expression))
            }
            ast::Kind::ParenthesizedExpression => {
                if let Some(expression) = source.expression(node) {
                    let skipped = ast::skip_outer_expressions(
                        source,
                        expression,
                        outer_expression_kinds_excluding_assertions_and_type_arguments(),
                    );
                    if ast::is_assertion_expression(source, skipped)
                        || ast::is_satisfies_expression(source, skipped)
                    {
                        let expression = self.visit_node(Some(expression));
                        return Some(
                            self.new_partially_emitted_expression(source, node, expression),
                        );
                    }
                }
                Some(self.visit_each_child(&node))
            }
            ast::Kind::JsxSelfClosingElement => {
                let tag_name = self.visit_node(source.tag_name(node));
                let attributes = self.visit_node(source.attributes(node));
                Some(
                    self.factory_mut()
                        .update_jsx_self_closing_element_from_store(
                            source, node, tag_name, None, attributes,
                        ),
                )
            }
            ast::Kind::JsxOpeningElement => {
                let tag_name = self.visit_node(source.tag_name(node));
                let attributes = self.visit_node(source.attributes(node));
                Some(self.factory_mut().update_jsx_opening_element_from_store(
                    source, node, tag_name, None, attributes,
                ))
            }
            ast::Kind::ImportEqualsDeclaration => {
                if source.is_type_only(node).unwrap_or(false) {
                    // elide type-only imports
                    return None;
                }
                Some(self.visit_each_child(&node))
            }
            ast::Kind::ImportDeclaration => {
                let Some(import_clause) = source.import_clause(node) else {
                    // Do not elide a side-effect only import declaration.
                    //  import "foo";
                    return Some(node);
                };
                let Some(import_clause) = self.visit_node(Some(import_clause)) else {
                    return None;
                };
                let modifiers = self.import_state.preserve_optional_source_modifier_list(
                    &mut self.emit_context.factory.node_factory,
                    source.source_modifiers(node),
                );
                let module_specifier = self.preserve_optional_node(source.module_specifier(node));
                let attributes = self.preserve_optional_node(source.attributes(node));
                Some(self.factory_mut().update_import_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    import_clause,
                    module_specifier,
                    attributes,
                ))
            }
            ast::Kind::ImportClause => {
                if source.is_type_only(node).unwrap_or(false) {
                    // Always elide type-only imports
                    return None;
                }
                let name = self.preserve_optional_node(source.name(node));
                let named_bindings = self.visit_node(source.named_bindings(node));
                if name.is_none() && named_bindings.is_none() {
                    // all import bindings were elided
                    return None;
                }
                Some(self.factory_mut().update_import_clause_from_store(
                    source,
                    node,
                    source.phase_modifier(node),
                    name,
                    named_bindings,
                ))
            }
            ast::Kind::NamedImports => {
                if source
                    .source_elements(node)
                    .map(|nodes| nodes.len())
                    .unwrap_or(0)
                    == 0
                {
                    // Do not elide a side-effect only import declaration.
                    return Some(node);
                }
                let elements = self
                    .visit_nodes_input(
                        (source.source_elements(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("named imports elements are required");
                if !self.compiler_options.verbatim_module_syntax.is_true()
                    && self.factory().emit_node_list_nodes(elements).is_empty()
                {
                    // all import specifiers were elided
                    return None;
                }
                Some(
                    self.factory_mut()
                        .update_named_imports_from_store(source, node, elements),
                )
            }
            ast::Kind::ImportSpecifier => {
                if source.is_type_only(node).unwrap_or(false) {
                    // elide type-only or unused imports
                    return None;
                }
                Some(node)
            }
            ast::Kind::ExportDeclaration => {
                if source.is_type_only(node).unwrap_or(false) {
                    // elide type-only exports
                    return None;
                }
                let export_clause = match source.export_clause(node) {
                    Some(export_clause) => Some(self.visit_node(Some(export_clause))?),
                    None => None,
                };
                let module_specifier = self.visit_node(source.module_specifier(node));
                let attributes = self.visit_node(source.attributes(node));
                Some(self.factory_mut().update_export_declaration_from_store(
                    source,
                    node,
                    None,
                    false,
                    export_clause,
                    module_specifier,
                    attributes,
                ))
            }
            ast::Kind::NamedExports => {
                if source
                    .source_elements(node)
                    .map(|nodes| nodes.len())
                    .unwrap_or(0)
                    == 0
                {
                    // Do not elide an empty export declaration.
                    return Some(node);
                }
                let elements = self
                    .visit_nodes_input(
                        (source.source_elements(node)).map(ast::SourceNodeListInput::from_source),
                    )
                    .expect("named exports elements are required");
                if !self.compiler_options.verbatim_module_syntax.is_true()
                    && self.factory().emit_node_list_nodes(elements).is_empty()
                {
                    // all export specifiers were elided
                    return None;
                }
                Some(
                    self.factory_mut()
                        .update_named_exports_from_store(source, node, elements),
                )
            }
            ast::Kind::ExportSpecifier => {
                if source.is_type_only(node).unwrap_or(false) {
                    // elide unused export
                    return None;
                }
                Some(node)
            }
            ast::Kind::EnumDeclaration => {
                if ast::is_enum_const(source, node) {
                    return Some(node);
                }
                Some(self.visit_each_child(&node))
            }
            ast::Kind::BinaryExpression => self.visit_binary_expression_chain(source, node),
            _ => Some(self.visit_each_child(&node)),
        }
    }

    fn strip_type_syntax_in_factory_store(&mut self, node: ast::Node) -> Option<ast::Node> {
        let kind = self.factory().store().kind(node);
        match kind {
            ast::Kind::ModuleDeclaration => {
                let should_elide = {
                    let store = self.factory().store();
                    self.should_elide_module_declaration(store, node)
                };
                if should_elide {
                    // TypeScript module declarations are elided if they are not instantiated or have no body
                    return Some(self.elide_statement(&node));
                }
                Some(self.visit_each_child(&node))
            }
            ast::Kind::ExpressionWithTypeArguments => {
                let expression = self.visit_node(self.current_expression(node));
                Some(
                    self.factory_mut()
                        .update_expression_with_type_arguments(node, expression, None),
                )
            }
            ast::Kind::Constructor => {
                if self.current_body(node).is_none() {
                    // TypeScript overloads are elided
                    return None;
                }
                let parameters = self
                    .visit_parameters_input(self.current_source_parameters_input(node))
                    .expect("constructor parameters are required");
                let body = self.visit_function_body(self.current_body(node));
                Some(
                    self.factory_mut().update_constructor_declaration(
                        node, None, None, parameters, None, None, body,
                    ),
                )
            }
            ast::Kind::PropertyDeclaration => {
                let has_ambient_or_abstract = self.current_has_syntactic_modifier(
                    node,
                    ast::ModifierFlags::AMBIENT | ast::ModifierFlags::ABSTRACT,
                );
                if self.compiler_options.experimental_decorators.is_true()
                    && has_ambient_or_abstract
                    && self.current_has_decorators(node)
                {
                    // declare/abstract props with decorators must be preserved until the decorator transform can process them and remove them
                } else if has_ambient_or_abstract {
                    // TypeScript `declare` fields are elided
                    return None;
                }
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let initializer = self.visit_node(self.current_initializer(node));
                let updated = self.factory_mut().update_property_declaration(
                    node,
                    modifiers,
                    name,
                    None,
                    None,
                    initializer,
                );
                Some(updated)
            }
            ast::Kind::MethodDeclaration => {
                let body = self.current_body(node);
                if self.current_node_is_missing(body) {
                    // TypeScript overloads are elided
                    return None;
                }
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let parameters = self
                    .visit_nodes_input(self.current_source_parameters_input(node))
                    .expect("method parameters are required");
                let body = self.visit_node(self.current_body(node));
                let asterisk_token = self.preserve_optional_node(self.current_asterisk_token(node));
                let updated = self.factory_mut().update_method_declaration(
                    node,
                    modifiers,
                    asterisk_token,
                    name,
                    None,
                    None,
                    parameters,
                    None,
                    None,
                    body,
                );
                Some(updated)
            }
            ast::Kind::GetAccessor => {
                let original_body = self.current_body(node);
                if self.current_node_is_missing(original_body)
                    && self.current_has_syntactic_modifier(node, ast::ModifierFlags::ABSTRACT)
                {
                    // Abstract accessors are elided
                    return None;
                }
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let parameters = self
                    .visit_nodes_input(self.current_source_parameters_input(node))
                    .expect("accessor parameters are required");
                let mut body = self.visit_node(self.current_body(node));
                if body.is_none() {
                    let empty_statements = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        Vec::<ast::Node>::new(),
                    );
                    body = Some(self.factory_mut().new_block(empty_statements, false));
                }
                let updated = self.factory_mut().update_get_accessor_declaration(
                    node, modifiers, name, None, parameters, None, None, body,
                );
                Some(updated)
            }
            ast::Kind::SetAccessor => {
                let original_body = self.current_body(node);
                if self.current_node_is_missing(original_body)
                    && self.current_has_syntactic_modifier(node, ast::ModifierFlags::ABSTRACT)
                {
                    // Abstract accessors are elided
                    return None;
                }
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let parameters = self
                    .visit_nodes_input(self.current_source_parameters_input(node))
                    .expect("accessor parameters are required");
                let mut body = self.visit_node(self.current_body(node));
                if body.is_none() {
                    let empty_statements = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        Vec::<ast::Node>::new(),
                    );
                    body = Some(self.factory_mut().new_block(empty_statements, false));
                }
                let updated = self.factory_mut().update_set_accessor_declaration(
                    node, modifiers, name, None, parameters, None, None, body,
                );
                Some(updated)
            }
            ast::Kind::HeritageClause => {
                let token = self
                    .current_token(node)
                    .expect("heritage clause should have token");
                if token == ast::Kind::ImplementsKeyword {
                    // TypeScript `implements` clauses are elided
                    return None;
                }
                let types = self
                    .visit_nodes_input(self.current_source_types_input(node))
                    .expect("heritage clause types are required");
                Some(
                    self.factory_mut()
                        .update_heritage_clause(node, token, types),
                )
            }
            ast::Kind::VariableDeclaration => {
                let name = self.visit_node(self.current_name(node));
                let initializer = self.visit_node(self.current_initializer(node));
                let type_node = self.current_type(node);
                let updated = self.factory_mut().update_variable_declaration(
                    node,
                    name,
                    None,
                    None,
                    initializer,
                );
                if let Some(type_node) = type_node {
                    let name = self
                        .factory()
                        .store()
                        .name(updated)
                        .expect("updated variable declaration should have a name");
                    self.emit_context.set_type_node(&name, &type_node);
                }
                Some(updated)
            }
            ast::Kind::Parameter => {
                if self.current_is_this_parameter(node) {
                    // TypeScript `this` parameters are elided
                    return None;
                }
                let is_parameter_property = self
                    .parent_node
                    .is_some_and(|parent| self.current_is_parameter_property(node, parent));
                let modifiers = if is_parameter_property {
                    // preserve parameter property modifiers to be handled by the runtime transformer
                    self.preserve_parameter_property_modifiers_input(
                        self.current_source_modifiers_input(node),
                    )
                } else {
                    // preserve decorators for the decorator transforms
                    self.preserve_parameter_decorators_input(
                        self.current_source_modifiers_input(node),
                    )
                };
                let name = self.visit_node(self.current_name(node));
                let initializer = self.visit_node(self.current_initializer(node));
                let dot_dot_dot_token =
                    self.preserve_optional_node(self.current_dot_dot_dot_token(node));
                let updated = self.factory_mut().update_parameter_declaration(
                    node,
                    modifiers,
                    dot_dot_dot_token,
                    name,
                    None,
                    None,
                    initializer,
                );
                Some(updated)
            }
            ast::Kind::ClassDeclaration => {
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let heritage_clauses =
                    self.visit_nodes_input(self.current_source_heritage_clauses_input(node));
                let members = self
                    .visit_nodes_input(self.current_source_members_input(node))
                    .expect("class members are required");
                Some(self.factory_mut().update_class_declaration(
                    node,
                    modifiers,
                    name,
                    None,
                    heritage_clauses,
                    members,
                ))
            }
            ast::Kind::ClassExpression => {
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let heritage_clauses =
                    self.visit_nodes_input(self.current_source_heritage_clauses_input(node));
                let members = self
                    .visit_nodes_input(self.current_source_members_input(node))
                    .expect("class members are required");
                Some(self.factory_mut().update_class_expression(
                    node,
                    modifiers,
                    name,
                    None,
                    heritage_clauses,
                    members,
                ))
            }
            ast::Kind::FunctionDeclaration => {
                let body = self.current_body(node);
                if self.current_node_is_missing(body) {
                    // TypeScript overloads are elided
                    return Some(self.elide_statement(&node));
                }
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let parameters = self
                    .visit_nodes_input(self.current_source_parameters_input(node))
                    .expect("function parameters are required");
                let body = self.visit_node(self.current_body(node));
                let asterisk_token = self.preserve_optional_node(self.current_asterisk_token(node));
                Some(self.factory_mut().update_function_declaration(
                    node,
                    modifiers,
                    asterisk_token,
                    name,
                    None,
                    parameters,
                    None,
                    None,
                    body,
                ))
            }
            ast::Kind::FunctionExpression => {
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let name = self.visit_node(self.current_name(node));
                let parameters = self
                    .visit_parameters_input(self.current_source_parameters_input(node))
                    .expect("function parameters are required");
                let body = self.visit_function_body(self.current_body(node));
                let asterisk_token = self.preserve_optional_node(self.current_asterisk_token(node));
                let updated = self.factory_mut().update_function_expression(
                    node,
                    modifiers,
                    asterisk_token,
                    name,
                    None,
                    parameters,
                    None,
                    None,
                    body,
                );
                Some(updated)
            }
            ast::Kind::ArrowFunction => {
                let modifiers =
                    self.visit_modifiers_input(self.current_source_modifiers_input(node));
                let parameters = self
                    .visit_nodes_input(self.current_source_parameters_input(node))
                    .expect("arrow function parameters are required");
                let equals_greater_than_token =
                    self.preserve_optional_node(self.current_equals_greater_than_token(node));
                let body = self.visit_node(self.current_body(node));
                Some(self.factory_mut().update_arrow_function(
                    node,
                    modifiers,
                    None,
                    parameters,
                    None,
                    None,
                    equals_greater_than_token,
                    body,
                ))
            }
            ast::Kind::CallExpression => {
                let expression = self.visit_node(self.current_expression(node));
                let question_dot_token =
                    self.preserve_optional_node(self.current_question_dot_token(node));
                let arguments = self
                    .visit_nodes_input(self.current_source_arguments_input(node))
                    .expect("call expression arguments are required");
                let flags = self.current_flags(node);
                Some(self.factory_mut().update_call_expression(
                    node,
                    expression,
                    question_dot_token,
                    None,
                    arguments,
                    flags,
                ))
            }
            ast::Kind::NewExpression => {
                let expression = self.visit_node(self.current_expression(node));
                let arguments = self.visit_nodes_input(self.current_source_arguments_input(node));
                Some(
                    self.factory_mut()
                        .update_new_expression(node, expression, None, arguments),
                )
            }
            ast::Kind::TaggedTemplateExpression => {
                let tag = self.visit_node(self.current_tag(node));
                let question_dot_token =
                    self.preserve_optional_node(self.current_question_dot_token(node));
                let template = self.visit_node(self.current_template(node));
                let flags = self.current_flags(node);
                Some(self.factory_mut().update_tagged_template_expression(
                    node,
                    tag,
                    question_dot_token,
                    None,
                    template,
                    flags,
                ))
            }
            ast::Kind::NonNullExpression
            | ast::Kind::TypeAssertionExpression
            | ast::Kind::AsExpression
            | ast::Kind::SatisfiesExpression => {
                let expression = self.visit_node(self.current_expression(node));
                Some(self.new_partially_emitted_expression_in_factory_store(node, expression))
            }
            ast::Kind::ParenthesizedExpression => {
                if let Some(expression) = self.current_expression(node) {
                    let should_partially_emit = {
                        let store = self.factory().store();
                        let skipped = ast::skip_outer_expressions(
                            store,
                            expression,
                            outer_expression_kinds_excluding_assertions_and_type_arguments(),
                        );
                        ast::is_assertion_expression(store, skipped)
                            || ast::is_satisfies_expression(store, skipped)
                    };
                    if should_partially_emit {
                        let expression = self.visit_node(Some(expression));
                        return Some(
                            self.new_partially_emitted_expression_in_factory_store(
                                node, expression,
                            ),
                        );
                    }
                }
                Some(self.visit_each_child(&node))
            }
            ast::Kind::JsxSelfClosingElement => {
                let tag_name = self.visit_node(self.current_tag_name(node));
                let attributes = self.visit_node(self.current_attributes(node));
                Some(
                    self.factory_mut()
                        .update_jsx_self_closing_element(node, tag_name, None, attributes),
                )
            }
            ast::Kind::JsxOpeningElement => {
                let tag_name = self.visit_node(self.current_tag_name(node));
                let attributes = self.visit_node(self.current_attributes(node));
                Some(
                    self.factory_mut()
                        .update_jsx_opening_element(node, tag_name, None, attributes),
                )
            }
            ast::Kind::ImportEqualsDeclaration => {
                if self.current_is_type_only(node) {
                    // elide type-only imports
                    return None;
                }
                Some(self.visit_each_child(&node))
            }
            ast::Kind::ImportDeclaration => {
                let Some(import_clause) = self.current_import_clause(node) else {
                    // Do not elide a side-effect only import declaration.
                    //  import "foo";
                    return Some(node);
                };
                let Some(import_clause) = self.visit_node(Some(import_clause)) else {
                    return None;
                };
                let modifiers = self
                    .current_source_modifiers_input(node)
                    .map(|modifiers| modifiers.as_modifier_list());
                let module_specifier =
                    self.preserve_optional_node(self.current_module_specifier(node));
                let attributes = self.preserve_optional_node(self.current_attributes(node));
                Some(self.factory_mut().update_import_declaration(
                    node,
                    modifiers,
                    import_clause,
                    module_specifier,
                    attributes,
                ))
            }
            ast::Kind::ImportClause => {
                if self.current_is_type_only(node) {
                    // Always elide type-only imports
                    return None;
                }
                let name = self.preserve_optional_node(self.current_name(node));
                let named_bindings = self.visit_node(self.current_named_bindings(node));
                if name.is_none() && named_bindings.is_none() {
                    // all import bindings were elided
                    return None;
                }
                let phase_modifier = self.current_phase_modifier(node);
                Some(self.factory_mut().update_import_clause(
                    node,
                    phase_modifier,
                    name,
                    named_bindings,
                ))
            }
            ast::Kind::NamedImports => {
                let element_count = self
                    .current_source_elements_input(node)
                    .map_or(0, |n| n.len());
                if element_count == 0 {
                    // Do not elide a side-effect only import declaration.
                    return Some(node);
                }
                let elements = self
                    .visit_nodes_input(self.current_source_elements_input(node))
                    .expect("named imports elements are required");
                if !self.compiler_options.verbatim_module_syntax.is_true()
                    && self.factory().emit_node_list_nodes(elements).is_empty()
                {
                    // all import specifiers were elided
                    return None;
                }
                Some(self.factory_mut().update_named_imports(node, elements))
            }
            ast::Kind::ImportSpecifier => {
                if self.current_is_type_only(node) {
                    // elide type-only or unused imports
                    return None;
                }
                Some(node)
            }
            ast::Kind::ExportDeclaration => {
                if self.current_is_type_only(node) {
                    // elide type-only exports
                    return None;
                }
                let export_clause = match self.current_export_clause(node) {
                    Some(export_clause) => Some(self.visit_node(Some(export_clause))?),
                    None => None,
                };
                let module_specifier = self.visit_node(self.current_module_specifier(node));
                let attributes = self.visit_node(self.current_attributes(node));
                Some(self.factory_mut().update_export_declaration(
                    node,
                    None,
                    false,
                    export_clause,
                    module_specifier,
                    attributes,
                ))
            }
            ast::Kind::NamedExports => {
                let element_count = self
                    .current_source_elements_input(node)
                    .map_or(0, |n| n.len());
                if element_count == 0 {
                    // Do not elide an empty export declaration.
                    return Some(node);
                }
                let elements = self
                    .visit_nodes_input(self.current_source_elements_input(node))
                    .expect("named exports elements are required");
                if !self.compiler_options.verbatim_module_syntax.is_true()
                    && self.factory().emit_node_list_nodes(elements).is_empty()
                {
                    // all export specifiers were elided
                    return None;
                }
                Some(self.factory_mut().update_named_exports(node, elements))
            }
            ast::Kind::ExportSpecifier => {
                if self.current_is_type_only(node) {
                    // elide unused export
                    return None;
                }
                Some(node)
            }
            ast::Kind::EnumDeclaration => {
                if self.current_is_enum_const(node) {
                    return Some(node);
                }
                Some(self.visit_each_child(&node))
            }
            _ => Some(self.visit_each_child(&node)),
        }
    }

    fn visit_binary_expression_chain(
        &mut self,
        source: &ast::AstStore,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let mut stack = vec![(node, false)];
        let mut visited_nodes = source.new_node_map::<Option<ast::Node>>();

        while let Some((current, expanded)) = stack.pop() {
            if visited_nodes.contains_key(current) {
                continue;
            }
            if !expanded {
                stack.push((current, true));
                if let Some(right) = source.right(current)
                    && source.kind(right) == ast::Kind::BinaryExpression
                {
                    stack.push((right, false));
                }
                if let Some(left) = source.left(current)
                    && source.kind(left) == ast::Kind::BinaryExpression
                {
                    stack.push((left, false));
                }
                continue;
            }

            let grandparent = if current == node {
                None
            } else {
                Some(self.push_node(current))
            };
            let modifiers = self.visit_modifiers_input(
                (source.source_modifiers(current)).map(ast::SourceModifierListInput::from_source),
            );
            let left =
                self.visit_binary_expression_child(source, source.left(current), &visited_nodes);
            let type_node =
                self.visit_binary_expression_child(source, source.r#type(current), &visited_nodes);
            let operator_token = self.visit_binary_expression_child(
                source,
                source.operator_token(current),
                &visited_nodes,
            );
            let right =
                self.visit_binary_expression_child(source, source.right(current), &visited_nodes);
            if let Some(grandparent) = grandparent {
                self.pop_node(grandparent);
            }

            let source_modifiers_input = source
                .source_modifiers(current)
                .map(ast::SourceModifierListInput::from_source);
            let source_unchanged = source.store_id() != self.factory().store().store_id()
                && self.preserved_source_modifier_list_input_matches(
                    source_modifiers_input.as_ref(),
                    modifiers,
                )
                && self.preserved_source_node_matches(source.left(current), left)
                && self.preserved_source_node_matches(source.r#type(current), type_node)
                && self
                    .preserved_source_node_matches(source.operator_token(current), operator_token)
                && self.preserved_source_node_matches(source.right(current), right);
            let updated = self.factory_mut().update_binary_expression_from_store(
                source,
                current,
                modifiers,
                left,
                type_node,
                operator_token,
                right,
            );
            let updated = if source_unchanged {
                self.record_preserved_node(current, updated)
            } else {
                updated
            };
            visited_nodes.insert(current, Some(updated));
        }

        visited_nodes.remove(node).flatten()
    }

    fn visit_binary_expression_child(
        &mut self,
        source: &ast::AstStore,
        child: Option<ast::Node>,
        visited_nodes: &ast::StoreNodeMap<Option<ast::Node>>,
    ) -> Option<ast::Node> {
        let child = child?;
        if source.kind(child) == ast::Kind::BinaryExpression {
            return visited_nodes.get_copied(child).flatten();
        }
        self.visit_node(Some(child))
    }

    fn new_partially_emitted_expression(
        &mut self,
        source: &ast::AstStore,
        original: ast::Node,
        expression: Option<ast::Node>,
    ) -> ast::Node {
        let partial = self
            .factory_mut()
            .new_partially_emitted_expression(expression);
        self.factory_mut()
            .place_transformed_node(partial, source.loc(original));
        self.emit_context.set_original_ex(&partial, &original, true);
        partial
    }

    fn new_partially_emitted_expression_in_factory_store(
        &mut self,
        original: ast::Node,
        expression: Option<ast::Node>,
    ) -> ast::Node {
        let loc = self.factory().store().loc(original);
        let partial = self
            .factory_mut()
            .new_partially_emitted_expression(expression);
        self.factory_mut().place_transformed_node(partial, loc);
        self.emit_context.set_original_ex(&partial, &original, true);
        partial
    }

    fn current_source_modifiers_input(
        &self,
        node: ast::Node,
    ) -> Option<ast::SourceModifierListInput> {
        self.factory()
            .store()
            .source_modifiers(node)
            .map(ast::SourceModifierListInput::from_source)
    }

    fn current_source_parameters_input(&self, node: ast::Node) -> Option<ast::SourceNodeListInput> {
        self.factory()
            .store()
            .source_parameters(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn current_source_heritage_clauses_input(
        &self,
        node: ast::Node,
    ) -> Option<ast::SourceNodeListInput> {
        self.factory()
            .store()
            .source_heritage_clauses(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn current_source_members_input(&self, node: ast::Node) -> Option<ast::SourceNodeListInput> {
        self.factory()
            .store()
            .source_members(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn current_source_types_input(&self, node: ast::Node) -> Option<ast::SourceNodeListInput> {
        self.factory()
            .store()
            .source_types(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn current_source_arguments_input(&self, node: ast::Node) -> Option<ast::SourceNodeListInput> {
        self.factory()
            .store()
            .source_arguments(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn current_source_elements_input(&self, node: ast::Node) -> Option<ast::SourceNodeListInput> {
        self.factory()
            .store()
            .source_elements(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn current_has_syntactic_modifier(&self, node: ast::Node, flags: ast::ModifierFlags) -> bool {
        ast::has_syntactic_modifier(self.factory().store(), node, flags)
    }

    fn current_has_decorators(&self, node: ast::Node) -> bool {
        ast::has_decorators(self.factory().store(), node)
    }

    fn current_node_is_missing(&self, node: Option<ast::Node>) -> bool {
        ast::node_is_missing(self.factory().store(), node)
    }

    fn current_is_this_parameter(&self, node: ast::Node) -> bool {
        ast::is_this_parameter(self.factory().store(), node)
    }

    fn current_is_parameter_property(&self, node: ast::Node, parent: ast::Node) -> bool {
        ast::is_parameter_property_declaration(self.factory().store(), node, parent)
    }

    fn current_is_type_only(&self, node: ast::Node) -> bool {
        self.factory().store().is_type_only(node).unwrap_or(false)
    }

    fn current_is_enum_const(&self, node: ast::Node) -> bool {
        ast::is_enum_const(self.factory().store(), node)
    }

    fn current_flags(&self, node: ast::Node) -> ast::NodeFlags {
        self.factory().store().flags(node)
    }

    fn current_expression(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().expression(node)
    }

    fn current_body(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().body(node)
    }

    fn current_name(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().name(node)
    }

    fn current_initializer(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().initializer(node)
    }

    fn current_asterisk_token(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().asterisk_token(node)
    }

    fn current_dot_dot_dot_token(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().dot_dot_dot_token(node)
    }

    fn current_equals_greater_than_token(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().equals_greater_than_token(node)
    }

    fn current_question_dot_token(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().question_dot_token(node)
    }

    fn current_token(&self, node: ast::Node) -> Option<ast::Kind> {
        self.factory().store().token(node)
    }

    fn current_type(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().r#type(node)
    }

    fn current_tag(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().tag(node)
    }

    fn current_template(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().template(node)
    }

    fn current_tag_name(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().tag_name(node)
    }

    fn current_attributes(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().attributes(node)
    }

    fn current_import_clause(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().import_clause(node)
    }

    fn current_module_specifier(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().module_specifier(node)
    }

    fn current_named_bindings(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().named_bindings(node)
    }

    fn current_phase_modifier(&self, node: ast::Node) -> Option<ast::Kind> {
        self.factory().store().phase_modifier(node)
    }

    fn current_export_clause(&self, node: ast::Node) -> Option<ast::Node> {
        self.factory().store().export_clause(node)
    }

    fn preserve_parameter_property_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let source = modifiers.store();
        let modifier_list = modifiers;
        let modifier_nodes = modifier_list.nodes();
        let preserved: Vec<_> = modifier_nodes
            .iter()
            .filter_map(|modifier| match source.kind(modifier) {
                kind if ast::is_parameter_property_modifier(kind) => {
                    Some(self.preserve_node(modifier))
                }
                ast::Kind::Decorator => self.visit_node(Some(modifier)),
                _ => None,
            })
            .collect();
        if preserved.is_empty() {
            None
        } else {
            let loc = modifier_nodes.loc();
            let range = modifier_nodes.range();
            let flags = modifier_list.modifier_flags();
            Some(
                self.factory_mut()
                    .new_modifier_list(loc, range, preserved, flags),
            )
        }
    }

    fn preserve_parameter_property_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let preserved: Vec<_> = modifiers
            .iter()
            .filter_map(|modifier| {
                let kind = self.store_for(modifier).kind(modifier);
                match kind {
                    kind if ast::is_parameter_property_modifier(kind) => {
                        Some(self.preserve_node(modifier))
                    }
                    ast::Kind::Decorator => self.visit_node(Some(modifier)),
                    _ => None,
                }
            })
            .collect();
        if preserved.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                preserved,
                modifiers.modifier_flags(),
            ))
        }
    }

    fn preserve_parameter_decorators(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let source = modifiers.store();
        let modifier_nodes = modifiers.nodes();
        let preserved: Vec<_> = modifier_nodes
            .iter()
            .filter_map(|modifier| {
                if source.kind(modifier) == ast::Kind::Decorator {
                    self.visit_node(Some(modifier))
                } else {
                    None
                }
            })
            .collect();
        if preserved.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                modifier_nodes.loc(),
                modifier_nodes.range(),
                preserved,
                modifiers.modifier_flags(),
            ))
        }
    }

    fn preserve_parameter_decorators_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let preserved: Vec<_> = modifiers
            .iter()
            .filter_map(|modifier| {
                if self.store_for(modifier).kind(modifier) == ast::Kind::Decorator {
                    self.visit_node(Some(modifier))
                } else {
                    None
                }
            })
            .collect();
        if preserved.is_empty() {
            None
        } else {
            Some(self.factory_mut().new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                preserved,
                modifiers.modifier_flags(),
            ))
        }
    }

    fn append_visited_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
        changed: &mut bool,
    ) {
        match visited {
            Some(visited) if self.visited_node_preserves_original(original, visited) => {
                out.push(self.preserve_node(original));
            }
            Some(visited) => {
                *changed = true;
                let store = self.store_for(visited);
                if store.kind(visited) == ast::Kind::SyntaxList {
                    let nodes = store
                        .syntax_list_children(visited)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    for node in nodes {
                        out.push(self.preserve_node(node));
                    }
                } else {
                    out.push(self.preserve_node(visited));
                }
            }
            None => *changed = true,
        }
    }

    fn visited_node_preserves_original(&self, original: ast::Node, visited: ast::Node) -> bool {
        if original.store_id() == self.factory().store().store_id() {
            original == visited
        } else {
            self.preserved_source_node_matches(Some(original), Some(visited))
        }
    }

    fn preserve_source_node_list_input(
        &mut self,
        nodes: &ast::SourceNodeListInput,
    ) -> ast::NodeList {
        if nodes.store_id() == self.factory().store().store_id() {
            return nodes.as_node_list();
        }
        self.import_state.preserve_source_node_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            nodes,
        )
    }

    fn preserve_source_modifier_list_input(
        &mut self,
        modifiers: &ast::SourceModifierListInput,
    ) -> ast::ModifierList {
        if modifiers.store_id() == self.factory().store().store_id() {
            return modifiers.as_modifier_list();
        }
        self.import_state.preserve_source_modifier_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            modifiers,
        )
    }

    fn preserve_source_raw_node_slice_input(
        &mut self,
        nodes: &ast::SourceRawNodeSliceInput,
    ) -> ast::RawNodeSlice {
        if nodes.store_id() == self.factory().store().store_id() {
            return nodes.as_raw_node_slice();
        }
        self.import_state.preserve_source_raw_node_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            nodes,
        )
    }

    fn lift_to_block_or_empty(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let Some(node) = node else {
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            return Some(self.factory_mut().new_block(statements, true));
        };
        Some(self.lift_to_block(node))
    }

    fn lift_to_block(&mut self, node: ast::Node) -> ast::Node {
        let store = self.store_for(node);
        let nodes = if store.kind(node) == ast::Kind::SyntaxList {
            store
                .syntax_list_children(node)
                .expect("SyntaxList should have children")
                .iter()
                .flatten()
                .collect::<Vec<_>>()
        } else {
            vec![node]
        };
        let nodes = nodes
            .into_iter()
            .map(|node| self.preserve_node(node))
            .collect::<Vec<_>>();
        if nodes.len() == 1 {
            nodes[0]
        } else {
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
            );
            self.factory_mut().new_block(statements, true)
        }
    }
}

fn outer_expression_kinds_excluding_assertions_and_type_arguments() -> ast::OuterExpressionKinds {
    let excluded = (ast::OuterExpressionKinds::ASSERTIONS
        | ast::OuterExpressionKinds::EXPRESSIONS_WITH_TYPE_ARGUMENTS)
        .0;
    ast::OuterExpressionKinds(ast::OuterExpressionKinds::ALL.0 & !excluded)
}

fn get_innermost_module_declaration_from_dotted_module(
    source: &ast::AstStore,
    module_declaration: ast::Node,
) -> ast::Node {
    let mut module_declaration = module_declaration;
    loop {
        let next = {
            let Some(body) = source.body(module_declaration) else {
                break;
            };
            if source.kind(body) != ast::Kind::ModuleDeclaration {
                break;
            }
            body
        };
        module_declaration = next;
    }
    module_declaration
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for TypeEraserRuntime<'_, 'source> {
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        self.import_state.preserved_node(self.factory(), source)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return node;
        }
        let source = self.source;
        let imported = self.import_state.preserve_node(
            source,
            &mut self.emit_context.factory.node_factory,
            node,
        );
        imported
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        let imported = self.preserve_node(imported);
        self.import_state.record_preserved_node(
            source.store_id(),
            &mut self.emit_context.factory.node_factory,
            source,
            imported,
        )
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        self.import_state
            .preserved_source_node_matches(self.factory(), source, output)
    }

    fn preserved_source_node_list_input_matches(
        &self,
        source: Option<&ast::SourceNodeListInput>,
        output: Option<ast::NodeList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_node_list());
        }
        self.import_state.preserved_source_node_list_input_matches(
            self.source,
            self.factory(),
            Some(source),
            output,
        )
    }

    fn preserved_source_modifier_list_input_matches(
        &self,
        source: Option<&ast::SourceModifierListInput>,
        output: Option<ast::ModifierList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_modifier_list());
        }
        self.import_state
            .preserved_source_modifier_list_input_matches(
                self.source,
                self.factory(),
                Some(source),
                output,
            )
    }

    fn preserved_source_raw_node_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawNodeSliceInput>,
        output: Option<ast::RawNodeSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_raw_node_slice());
        }
        self.import_state
            .preserved_source_raw_node_slice_input_matches(
                self.source,
                self.factory(),
                Some(source),
                output,
            )
    }

    fn preserved_source_raw_string_slice_input_matches(
        &self,
        source: Option<&ast::SourceRawStringSliceInput>,
        output: Option<ast::RawStringSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        if source.store_id() == self.factory().store().store_id() {
            return output == Some(source.as_raw_string_slice());
        }
        self.import_state
            .preserved_source_raw_string_slice_input_matches(
                self.source,
                self.factory(),
                Some(source),
                output,
            )
    }

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: Option<ast::NodeList>,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            if source_unchanged {
                return node;
            }
            return self
                .emit_context
                .factory
                .node_factory
                .update_source_file_in_current_store(
                    node,
                    statements.expect("source file statements cannot be removed"),
                    end_of_file_token,
                );
        }
        let source = self.source;
        if source_unchanged {
            let imported = self.preserve_node(node);
            return self.record_preserved_node(node, imported);
        }
        self.import_state.update_source_file_from_store(
            source,
            &mut self.emit_context.factory.node_factory,
            node,
            statements.expect("source file statements cannot be removed"),
            end_of_file_token,
        )
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let mut visited = self.visit(&node)?;
        let store = self.store_for(visited);
        if store.kind(visited) == ast::Kind::SyntaxList {
            let mut nodes = store
                .syntax_list_children(visited)
                .expect("SyntaxList should have children")
                .iter();
            let visited_slot = nodes
                .next()
                .expect("expected only a single node to be written to output");
            assert!(
                nodes.next().is_none(),
                "expected only a single node to be written to output"
            );
            visited = visited_slot?;
        }
        Some(self.preserve_node(visited))
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.visit_node(node)
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let old_flags = self.emit_context.begin_visit_parameters();
        let mut visited = Vec::with_capacity(nodes.len());
        let mut changed = false;
        for node in nodes.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        let (visited, changed) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        if changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                nodes.loc(),
                nodes.range(),
                visited,
                nodes.has_trailing_comma(),
            ))
        } else {
            Some(self.preserve_source_node_list_input(&nodes))
        }
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let updated = self.visit_node(node);
        self.emit_context.finish_visit_function_body(updated)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node?;
        self.emit_context.begin_visit_iteration_body();
        let updated = self.visit_embedded_statement(node);
        self.emit_context.finish_visit_iteration_body(updated)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        match node {
            Some(node) => {
                let visited = self.visit(&node);
                let lifted = self.lift_to_block_or_empty(visited);
                let updated = self
                    .emit_context
                    .finish_visit_embedded_statement(&node, lifted);
                updated.map(|updated| self.preserve_node(updated))
            }
            None => None,
        }
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for TypeEraserRuntime<'_, 'source> {}

#[cfg(test)]
mod tests {
    use super::*;
    use ts_parser as parser;

    fn parse_typescript(text: &str) -> ast::SourceFile {
        parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/typeeraser.ts".to_string(),
                path: "/typeeraser.ts".to_string(),
                ..Default::default()
            },
            text.to_string(),
            core::ScriptKind::TS,
        )
    }

    #[test]
    fn type_eraser_strips_type_syntax_from_factory_store_root() {
        let source_file = parse_typescript("");
        let mut emit_context = printer::new_emit_context();
        let root = {
            let factory = &mut emit_context.factory.node_factory;
            let expression = factory.new_numeric_literal("1", ast::TokenFlags::NONE);
            let type_node = factory.new_keyword_type_node(ast::Kind::NumberKeyword);
            let as_expression = factory.new_as_expression(Some(expression), Some(type_node));
            let statement = factory.new_expression_statement(Some(as_expression));
            let statements = factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                vec![statement],
            );
            factory.new_source_file(
                ast::SourceFileParseOptions {
                    file_name: "/typeeraser.ts".to_string(),
                    path: "/typeeraser.ts".to_string(),
                    ..Default::default()
                },
                "1 as number",
                statements,
                None,
            )
        };

        let root = visit_source_file_root(
            &source_file,
            root,
            &mut emit_context,
            &core::CompilerOptions::default(),
        );
        let store = emit_context.factory.node_factory.store();
        let statement = store.parser_access().source_file_statement_nodes(root)[0];
        let expression = store
            .expression(statement)
            .expect("expression statement should have expression");

        assert_eq!(
            store.kind(expression),
            ast::Kind::PartiallyEmittedExpression
        );
        let inner = store
            .expression(expression)
            .expect("partial expression should keep runtime expression");
        assert_eq!(store.kind(inner), ast::Kind::NumericLiteral);
    }
}
