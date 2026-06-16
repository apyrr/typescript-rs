use std::collections::HashMap;

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_evaluator as evaluator;
use ts_printer as printer;

use crate::modifiervisitor;
use crate::tstransforms::utilities::{
    constant_expression_from_number, constant_expression_from_string,
};
use crate::utilities::{is_generated_identifier, is_identifier_reference, is_local_name};

#[derive(Clone, Default)]
pub struct RuntimeSyntaxResolverFacts {
    enum_member_values: HashMap<core::TextRange, evaluator::Result>,
    referenced_export_containers: HashMap<core::TextRange, ast::Node>,
}

impl RuntimeSyntaxResolverFacts {
    fn enum_member_value(&self, store: &ast::AstStore, member: ast::Node) -> evaluator::Result {
        self.enum_member_values
            .get(&store.loc(member))
            .cloned()
            .unwrap_or_default()
    }

    fn referenced_export_container(
        &self,
        store: &ast::AstStore,
        node: ast::Node,
    ) -> Option<ast::Node> {
        self.referenced_export_containers
            .get(&store.loc(node))
            .copied()
    }
}

pub fn collect_runtime_syntax_resolver_facts(
    source_file: &ast::SourceFile,
    _emit_context: &mut printer::EmitContext,
    resolver: &mut dyn printer::EmitResolver,
) -> RuntimeSyntaxResolverFacts {
    let mut facts = RuntimeSyntaxResolverFacts::default();
    let store = source_file.store();
    let mut stack = vec![source_file.root()];
    while let Some(node) = stack.pop() {
        if store.kind(node) == ast::Kind::EnumMember {
            facts
                .enum_member_values
                .insert(store.loc(node), resolver.get_enum_member_value(node));
        } else if ast::is_identifier(store, node)
            && let Some(container) = resolver.get_referenced_export_container(node, false)
            && store.kind(container) != ast::Kind::SourceFile
        {
            facts
                .referenced_export_containers
                .insert(store.loc(node), container);
        }
        let _ = store.for_each_present_child(node, |child| {
            stack.push(child);
            std::ops::ControlFlow::Continue(())
        });
    }
    facts
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSyntaxAction {
    Keep,
    Elide,
    VisitEnum,
    VisitNamespace,
    VisitClass,
    VisitFunction,
    VisitVariable,
    VisitImportEquals,
    VisitIdentifier,
}

pub fn runtime_syntax_action_for_kind(
    kind: ast::Kind,
    inside_namespace: bool,
) -> RuntimeSyntaxAction {
    match kind {
        ast::Kind::PublicKeyword
        | ast::Kind::PrivateKeyword
        | ast::Kind::ProtectedKeyword
        | ast::Kind::ReadonlyKeyword
        | ast::Kind::OverrideKeyword => RuntimeSyntaxAction::Elide,
        ast::Kind::EnumDeclaration => RuntimeSyntaxAction::VisitEnum,
        ast::Kind::ModuleDeclaration => RuntimeSyntaxAction::VisitNamespace,
        ast::Kind::ClassDeclaration | ast::Kind::ClassExpression | ast::Kind::Constructor => {
            RuntimeSyntaxAction::VisitClass
        }
        ast::Kind::FunctionDeclaration => RuntimeSyntaxAction::VisitFunction,
        ast::Kind::VariableStatement => RuntimeSyntaxAction::VisitVariable,
        ast::Kind::ExportDeclaration | ast::Kind::ImportDeclaration | ast::Kind::ImportClause
            if inside_namespace =>
        {
            RuntimeSyntaxAction::Elide
        }
        ast::Kind::ImportEqualsDeclaration => RuntimeSyntaxAction::VisitImportEquals,
        ast::Kind::Identifier | ast::Kind::ShorthandPropertyAssignment => {
            RuntimeSyntaxAction::VisitIdentifier
        }
        _ => RuntimeSyntaxAction::Keep,
    }
}

pub(crate) fn visit_source_file_root(
    file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    facts: &RuntimeSyntaxResolverFacts,
) -> ast::Node {
    let mut tx = RuntimeSyntaxTransformer::new(emit_context, compiler_options, file.store(), facts);
    tx.visit_source_file_root(root)
}

struct RuntimeSyntaxTransformer<'ctx, 'src> {
    emit_context: &'ctx mut printer::EmitContext,
    compiler_options: &'ctx core::CompilerOptions,
    source: &'src ast::AstStore,
    facts: &'ctx RuntimeSyntaxResolverFacts,
    parent_node: Option<ast::Node>,
    current_node: Option<ast::Node>,
    current_source_file: Option<ast::Node>,
    current_scope: Option<ast::Node>,
    current_scope_first_declarations_of_name: Option<HashMap<String, ast::NodeId>>,
    current_namespace: Option<ast::Node>,
    current_namespace_container_name: Option<ast::Node>,
    current_enum: Option<ast::Node>,
    import_state: ast::AstImportState,
}

impl<'ctx, 'src> RuntimeSyntaxTransformer<'ctx, 'src> {
    fn new(
        emit_context: &'ctx mut printer::EmitContext,
        compiler_options: &'ctx core::CompilerOptions,
        source: &'src ast::AstStore,
        facts: &'ctx RuntimeSyntaxResolverFacts,
    ) -> Self {
        Self {
            emit_context,
            compiler_options,
            source,
            facts,
            parent_node: None,
            current_node: None,
            current_source_file: None,
            current_scope: None,
            current_scope_first_declarations_of_name: None,
            current_namespace: None,
            current_namespace_container_name: None,
            current_enum: None,
            import_state: ast::AstImportState::new(),
        }
    }

    fn visit_source_file_root(&mut self, root: ast::Node) -> ast::Node {
        let grandparent_node = self.push_node(root);
        let (saved_scope, saved_first) = self.push_scope(root);

        let (statements_loc, statements_range, statement_nodes, end_of_file_token) = {
            let source = self.store_for(root);
            let source_statements = source
                .source_statements(root)
                .expect("source file should have statements");
            (
                source_statements.loc(),
                source_statements.range(),
                source_statements.iter().collect::<Vec<_>>(),
                source.as_source_file(root).end_of_file_token(),
            )
        };
        self.emit_context.start_variable_environment();
        let statements = self.visit_statement_list(&statement_nodes);
        let declarations = self.emit_context.end_variable_environment();
        let (statements, _) = self
            .emit_context
            .merge_environment_for_resolved_nodes(&statements, &declarations);
        let statement_list =
            self.factory()
                .new_node_list(statements_loc, statements_range, statements);
        let end_of_file_token = self.preserve_optional_node(end_of_file_token);
        let updated = if self.is_factory_node(root) {
            self.factory().update_source_file_in_current_store(
                root,
                Some(statement_list),
                end_of_file_token,
            )
        } else {
            let source = self.source;
            let source_data = source.as_source_file(root);
            self.factory().update_source_file_from_store(
                source,
                root,
                source_data,
                Some(statement_list),
                end_of_file_token,
            )
        };

        self.pop_scope(saved_scope, saved_first);
        self.pop_node(grandparent_node);
        updated
    }

    fn is_factory_node(&self, node: ast::Node) -> bool {
        node.store_id() == self.node_factory().store().store_id()
    }

    fn factory(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn node_factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn printer_factory(&mut self) -> &mut printer::NodeFactory {
        &mut self.emit_context.factory
    }

    fn store(&self) -> &ast::AstStore {
        self.emit_context.factory.store()
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return node;
        }
        assert_eq!(
            node.store_id(),
            self.source.store_id(),
            "transform traversal cannot read unrelated AST store"
        );
        let source = self.source;
        let factory = &mut self.emit_context.factory.node_factory;
        let imported = self.import_state.preserve_node(source, factory, node);
        self.copy_preserved_emit_metadata(node, imported);
        imported
    }

    fn copy_preserved_emit_metadata(&mut self, source: ast::Node, imported: ast::Node) {
        if source == imported {
            return;
        }

        let loc = {
            let store = self.store_for(source);
            store.loc(source)
        };
        let comment_range = self.emit_context.comment_range(&source);
        if comment_range != loc {
            self.emit_context
                .set_comment_range(&imported, comment_range);
        }
        let source_map_range = self.emit_context.source_map_range(&source);
        if source_map_range != loc {
            self.emit_context
                .set_source_map_range(&imported, source_map_range);
        }
    }

    fn preserve_optional_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.preserve_node(node))
    }

    fn clone_node_from_source(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        self.printer_factory().clone_node_with_hooks(source, node)
    }

    fn preserve_node_list(
        &mut self,
        list: Option<ast::SourceNodeList<'_>>,
    ) -> Option<ast::NodeList> {
        let factory = &mut self.emit_context.factory.node_factory;
        self.import_state
            .preserve_optional_source_node_list(factory, list)
    }

    fn import_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        let factory = &mut self.emit_context.factory.node_factory;
        self.import_state
            .preserve_optional_source_modifier_list(factory, modifiers)
    }

    fn import_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        modifiers.map(|modifiers| self.preserve_source_modifier_list_input(&modifiers))
    }

    fn extract_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
        allowed: ast::ModifierFlags,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let loc = modifiers.loc();
        let range = modifiers.range();
        let modifier_flags = modifiers.modifier_flags();
        let source = self.source_store_for_store_id(modifiers.store_id());
        let filtered: Vec<_> = modifiers
            .iter()
            .filter(|modifier| {
                modifiervisitor::modifier_is_allowed(source.kind(*modifier), allowed)
            })
            .collect();
        let filtered: Vec<ast::Node> = filtered
            .into_iter()
            .map(|modifier| self.preserve_node(modifier))
            .collect();
        Some(
            self.factory()
                .new_modifier_list(loc, range, filtered, modifier_flags),
        )
    }

    fn strip_parameter_property_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let loc = modifiers.loc();
        let range = modifiers.range();
        let modifier_flags = modifiers.modifier_flags();
        let source = self.source_store_for_store_id(modifiers.store_id());
        let filtered: Vec<_> = modifiers
            .iter()
            .filter(|modifier| {
                !matches!(
                    source.kind(*modifier),
                    ast::Kind::PublicKeyword
                        | ast::Kind::PrivateKeyword
                        | ast::Kind::ProtectedKeyword
                        | ast::Kind::ReadonlyKeyword
                        | ast::Kind::OverrideKeyword
                )
            })
            .collect();
        let filtered: Vec<ast::Node> = filtered
            .into_iter()
            .map(|modifier| self.preserve_node(modifier))
            .collect();
        if filtered.is_empty() {
            None
        } else {
            Some(
                self.factory()
                    .new_modifier_list(loc, range, filtered, modifier_flags),
            )
        }
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.node_factory(), node)
    }

    fn get_local_name_ex(
        &mut self,
        node: ast::Node,
        opts: printer::AssignedNameOptions,
    ) -> ast::Node {
        self.get_name_with_emit_flags(node, printer::EF_LOCAL_NAME, opts)
    }

    fn get_export_name_ex(
        &mut self,
        node: ast::Node,
        opts: printer::AssignedNameOptions,
    ) -> ast::Node {
        self.get_name_with_emit_flags(node, printer::EF_EXPORT_NAME, opts)
    }

    fn get_declaration_name_ex(
        &mut self,
        node: ast::Node,
        opts: printer::NameOptions,
    ) -> ast::Node {
        self.get_name_with_emit_flags(
            node,
            printer::EF_NONE,
            printer::AssignedNameOptions {
                allow_comments: opts.allow_comments,
                allow_source_maps: opts.allow_source_maps,
                ignore_assigned_name: false,
            },
        )
    }

    fn get_name_with_emit_flags(
        &mut self,
        node: ast::Node,
        emit_flags: printer::EmitFlags,
        opts: printer::AssignedNameOptions,
    ) -> ast::Node {
        let node_name = {
            let source = self.store_for(node);
            if opts.ignore_assigned_name {
                ast::get_non_assigned_name_of_declaration(source, node)
            } else {
                ast::get_name_of_declaration(source, Some(node))
            }
        };
        if let Some(node_name) = node_name {
            let name = self.clone_node_from_source(node_name);
            let mut emit_flags = emit_flags;
            if !opts.allow_comments {
                emit_flags |= printer::EF_NO_COMMENTS;
            }
            if !opts.allow_source_maps {
                emit_flags |= printer::EF_NO_SOURCE_MAP;
            }
            self.emit_context.mark_emit_node(&name, emit_flags);
            return name;
        }

        self.emit_context.new_generated_name_for_node(node)
    }

    fn get_external_module_or_namespace_export_name(
        &mut self,
        namespace: Option<&ast::Node>,
        node: ast::Node,
        allow_comments: bool,
        allow_source_maps: bool,
    ) -> ast::Node {
        if let Some(namespace) = namespace {
            let has_export = {
                let source = self.store_for(node);
                ast::has_syntactic_modifier(source, node, ast::ModifierFlags::EXPORT)
            };
            if has_export {
                let name_opts = printer::NameOptions {
                    allow_comments,
                    allow_source_maps,
                };
                let declaration_name = self.get_declaration_name_ex(node, name_opts);
                let source = self.source;
                return self.printer_factory().get_namespace_member_name(
                    source,
                    namespace,
                    &declaration_name,
                    name_opts,
                );
            }
        }

        self.get_export_name_ex(
            node,
            printer::AssignedNameOptions {
                allow_comments,
                allow_source_maps,
                ignore_assigned_name: false,
            },
        )
    }

    fn new_generated_name_for_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.node_factory().store().store_id() {
            if let Some(parse_node) = self.emit_context.parse_node(&node)
                && parse_node.store_id() == self.source.store_id()
            {
                let source = self.source;
                return self
                    .printer_factory()
                    .new_generated_name_for_node(source, &parse_node);
            }
        }
        self.emit_context.new_generated_name_for_node(node)
    }

    fn new_string_literal_from_node(&mut self, node: ast::Node) -> ast::Node {
        if self.is_factory_node(node) {
            let text = self.node_factory().store().text(node);
            self.factory()
                .new_string_literal(text, ast::TokenFlags::NONE)
        } else {
            let source = self.source;
            self.printer_factory()
                .new_string_literal_from_node(source, &node)
        }
    }

    fn create_expression_from_entity_name(&mut self, node: ast::Node) -> ast::Node {
        if !self.is_factory_node(node) {
            let source = self.source;
            return self
                .printer_factory()
                .create_expression_from_entity_name(source, &node);
        }

        let (is_qualified_name, left, right, loc) = {
            let source = self.store();
            (
                ast::is_qualified_name(source, node),
                source.left(node),
                source.right(node),
                source.loc(node),
            )
        };
        if is_qualified_name {
            let left = self.create_expression_from_entity_name(
                left.expect("qualified name should have left node"),
            );
            let right =
                self.clone_node_from_source(right.expect("qualified name should have right node"));
            let prop_access = self.factory().new_property_access_expression(
                left,
                None::<ast::Node>,
                right,
                ast::NodeFlags::NONE,
            );
            self.factory().place_transformed_node(prop_access, loc);
            return prop_access;
        }

        self.clone_node_from_source(node)
    }

    fn update_heritage_clause(
        &mut self,
        node: ast::Node,
        token: ast::Kind,
        types: ast::NodeList,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_heritage_clause(node, token, types)
        } else {
            let source = self.source;
            self.factory()
                .update_heritage_clause_from_store(source, node, token, types)
        }
    }

    fn update_expression_with_type_arguments(
        &mut self,
        node: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory()
                .update_expression_with_type_arguments(node, expression, None)
        } else {
            let source = self.source;
            self.factory()
                .update_expression_with_type_arguments_from_store(source, node, expression, None)
        }
    }

    fn update_module_block(&mut self, node: ast::Node, statements: ast::NodeList) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_module_block(node, statements)
        } else {
            let source = self.source;
            self.factory()
                .update_module_block_from_store(source, node, statements)
        }
    }

    fn update_function_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        asterisk_token: Option<ast::Node>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_function_declaration(
                node,
                modifiers,
                asterisk_token,
                name,
                None::<ast::NodeList>,
                parameters,
                None,
                None,
                body,
            )
        } else {
            let source = self.source;
            self.factory().update_function_declaration_from_store(
                source,
                node,
                modifiers,
                asterisk_token,
                name,
                None::<ast::NodeList>,
                parameters,
                None,
                None,
                body,
            )
        }
    }

    fn update_class_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        name: Option<ast::Node>,
        heritage_clauses: Option<ast::NodeList>,
        members: ast::NodeList,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_class_declaration(
                node,
                modifiers,
                name,
                None::<ast::NodeList>,
                heritage_clauses,
                members,
            )
        } else {
            let source = self.source;
            self.factory().update_class_declaration_from_store(
                source,
                node,
                modifiers,
                name,
                None::<ast::NodeList>,
                heritage_clauses,
                members,
            )
        }
    }

    fn update_class_expression(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        name: Option<ast::Node>,
        heritage_clauses: Option<ast::NodeList>,
        members: ast::NodeList,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_class_expression(
                node,
                modifiers,
                name,
                None::<ast::NodeList>,
                heritage_clauses,
                members,
            )
        } else {
            let source = self.source;
            self.factory().update_class_expression_from_store(
                source,
                node,
                modifiers,
                name,
                None::<ast::NodeList>,
                heritage_clauses,
                members,
            )
        }
    }

    fn update_constructor_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_constructor_declaration(
                node,
                modifiers,
                None::<ast::NodeList>,
                parameters,
                None,
                None,
                body,
            )
        } else {
            let source = self.source;
            self.factory().update_constructor_declaration_from_store(
                source,
                node,
                modifiers,
                None::<ast::NodeList>,
                parameters,
                None,
                None,
                body,
            )
        }
    }

    fn update_parameter_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        dot_dot_dot_token: Option<ast::Node>,
        name: Option<ast::Node>,
        question_token: Option<ast::Node>,
        type_node: Option<ast::Node>,
        initializer: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_parameter_declaration(
                node,
                modifiers,
                dot_dot_dot_token,
                name,
                question_token,
                type_node,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory().update_parameter_declaration_from_store(
                source,
                node,
                modifiers,
                dot_dot_dot_token,
                name,
                question_token,
                type_node,
                initializer,
            )
        }
    }

    fn update_shorthand_property_assignment(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        name: Option<ast::Node>,
        postfix_token: Option<ast::Node>,
        equals_token: Option<ast::Node>,
        initializer: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory().update_shorthand_property_assignment(
                node,
                modifiers,
                name,
                postfix_token,
                None,
                equals_token,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory()
                .update_shorthand_property_assignment_from_store(
                    source,
                    node,
                    modifiers,
                    name,
                    postfix_token,
                    None,
                    equals_token,
                    initializer,
                )
        }
    }

    fn new_synth_node_list(&mut self, nodes: impl IntoIterator<Item = ast::Node>) -> ast::NodeList {
        self.factory().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            nodes,
        )
    }

    fn new_syntax_list_from_vec(&mut self, nodes: Vec<ast::Node>) -> ast::Node {
        self.factory().new_syntax_list(nodes)
    }

    fn flatten_node(&self, node: ast::Node) -> Vec<ast::Node> {
        let store = self.store_for(node);
        if store.kind(node) == ast::Kind::SyntaxList {
            store
                .syntax_list_children(node)
                .expect("syntax list should have children")
                .iter()
                .flatten()
                .collect()
        } else {
            vec![node]
        }
    }

    fn flatten_node_into_factory(&mut self, node: ast::Node) -> Vec<ast::Node> {
        self.flatten_node(node)
            .into_iter()
            .map(|node| self.preserve_node(node))
            .collect()
    }

    fn strip_runtime_modifiers_for_var(
        &mut self,
        modifiers: Option<(
            core::TextRange,
            core::TextRange,
            Vec<ast::Node>,
            ast::ModifierFlags,
        )>,
        inside_namespace: bool,
    ) -> Option<ast::ModifierList> {
        let (loc, range, modifier_nodes, modifier_flags) = modifiers?;
        let mut filtered = Vec::new();
        for modifier in modifier_nodes {
            let keep = {
                let kind = self.store_for(modifier).kind(modifier);
                !matches!(
                    kind,
                    ast::Kind::PublicKeyword
                        | ast::Kind::PrivateKeyword
                        | ast::Kind::ProtectedKeyword
                        | ast::Kind::ReadonlyKeyword
                        | ast::Kind::OverrideKeyword
                        | ast::Kind::AbstractKeyword
                        | ast::Kind::ConstKeyword
                        | ast::Kind::DeclareKeyword
                        | ast::Kind::Decorator
                ) && !(inside_namespace && kind == ast::Kind::ExportKeyword)
            };
            if keep {
                filtered.push(self.preserve_node(modifier));
            }
        }
        if filtered.is_empty() {
            None
        } else {
            Some(
                self.factory()
                    .new_modifier_list(loc, range, filtered, modifier_flags),
            )
        }
    }

    fn strip_parameter_property_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let source = modifiers.store();
        let modifier_list = modifiers;
        let modifier_nodes = modifier_list.nodes();
        let filtered: Vec<_> = modifier_nodes
            .iter()
            .filter(|modifier| {
                !matches!(
                    source.kind(*modifier),
                    ast::Kind::PublicKeyword
                        | ast::Kind::PrivateKeyword
                        | ast::Kind::ProtectedKeyword
                        | ast::Kind::ReadonlyKeyword
                        | ast::Kind::OverrideKeyword
                )
            })
            .map(|modifier| self.preserve_node(modifier))
            .collect();
        if filtered.is_empty() {
            None
        } else {
            Some(self.factory().new_modifier_list(
                modifier_nodes.loc(),
                modifier_nodes.range(),
                filtered,
                modifier_list.modifier_flags(),
            ))
        }
    }

    fn push_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        let grandparent_node = self.parent_node;
        self.parent_node = self.current_node;
        self.current_node = Some(node);
        grandparent_node
    }

    fn pop_node(&mut self, grandparent_node: Option<ast::Node>) {
        self.current_node = self.parent_node.clone();
        self.parent_node = grandparent_node;
    }

    fn push_scope(
        &mut self,
        node: ast::Node,
    ) -> (Option<ast::Node>, Option<HashMap<String, ast::NodeId>>) {
        let saved_current_scope = self.current_scope.clone();
        let saved_current_scope_first_declarations_of_name =
            self.current_scope_first_declarations_of_name.clone();
        match self.store_for(node).kind(node) {
            ast::Kind::SourceFile => {
                self.current_scope = Some(node);
                self.current_source_file = Some(node);
                self.current_scope_first_declarations_of_name = None;
            }
            ast::Kind::CaseBlock | ast::Kind::ModuleBlock | ast::Kind::Block => {
                self.current_scope = Some(node);
                self.current_scope_first_declarations_of_name = None;
            }
            ast::Kind::FunctionDeclaration
            | ast::Kind::ClassDeclaration
            | ast::Kind::VariableStatement => {
                self.record_declaration_in_scope(node);
            }
            _ => {}
        }
        (
            saved_current_scope,
            saved_current_scope_first_declarations_of_name,
        )
    }

    fn pop_scope(
        &mut self,
        saved_current_scope: Option<ast::Node>,
        saved_current_scope_first_declarations_of_name: Option<HashMap<String, ast::NodeId>>,
    ) {
        let changed = match (&self.current_scope, &saved_current_scope) {
            (Some(current), Some(saved)) => *current != *saved,
            (None, None) => false,
            _ => true,
        };
        if changed {
            self.current_scope_first_declarations_of_name =
                saved_current_scope_first_declarations_of_name;
        }
        self.current_scope = saved_current_scope;
    }

    fn visit_statement_list(&mut self, statements: &[ast::Node]) -> Vec<ast::Node> {
        let mut result = Vec::new();
        for &statement in statements {
            match self.visit(statement) {
                None => {}
                Some(node) if self.store_for(node).kind(node) == ast::Kind::SyntaxList => {
                    result.extend(self.flatten_node_into_factory(node));
                }
                Some(node) => result.push(self.preserve_node(node)),
            }
        }
        result
    }

    fn visit(&mut self, node: ast::Node) -> Option<ast::Node> {
        let grandparent_node = self.push_node(node);
        let (saved_scope, saved_first) = self.push_scope(node);

        let store = self.store_for(node);
        let kind = store.kind(node);
        let facts = store.subtree_facts(node);
        if node.store_id() == self.store().store_id()
            && ast::is_token_kind(kind)
            && !matches!(kind, ast::Kind::Identifier | ast::Kind::PrivateIdentifier)
        {
            self.pop_scope(saved_scope, saved_first);
            self.pop_node(grandparent_node);
            return Some(node);
        }
        let result = if matches!(
            kind,
            ast::Kind::ExportDeclaration | ast::Kind::ImportDeclaration | ast::Kind::ImportClause
        ) && self.should_elide_import_or_export_in_namespace()
        {
            // do not emit ES6 imports and exports since they are illegal inside a namespace
            None
        } else if kind == ast::Kind::EnumDeclaration {
            Some(self.visit_enum_declaration(node))
        } else if kind == ast::Kind::ModuleDeclaration {
            Some(self.visit_module_declaration(node))
        } else if kind == ast::Kind::ClassDeclaration {
            Some(self.visit_class_declaration(node))
        } else if kind == ast::Kind::ClassExpression {
            Some(self.visit_class_expression(node))
        } else if kind == ast::Kind::Constructor {
            Some(self.visit_constructor_declaration(node))
        } else if kind == ast::Kind::ImportEqualsDeclaration {
            self.visit_import_equals_declaration_with_namespace_rules(node)
        } else if !facts.contains(ast::SubtreeFacts::CONTAINS_TYPE_SCRIPT)
            && ((self.current_namespace.is_none() && self.current_enum.is_none())
                || !facts.contains(ast::SubtreeFacts::CONTAINS_IDENTIFIER))
        {
            Some(node)
        } else {
            match kind {
                ast::Kind::PublicKeyword
                | ast::Kind::PrivateKeyword
                | ast::Kind::ProtectedKeyword
                | ast::Kind::ReadonlyKeyword
                | ast::Kind::OverrideKeyword => None,
                ast::Kind::ModuleBlock => Some(self.visit_module_block(node)),
                ast::Kind::FunctionDeclaration => Some(self.visit_function_declaration(node)),
                ast::Kind::VariableStatement => self.visit_variable_statement(node),
                ast::Kind::HeritageClause => self.visit_heritage_clause(node),
                ast::Kind::ExpressionWithTypeArguments => {
                    Some(self.visit_expression_with_type_arguments(node))
                }
                ast::Kind::ShorthandPropertyAssignment => {
                    Some(self.visit_shorthand_property_assignment(node))
                }
                ast::Kind::Identifier => Some(self.visit_identifier(node)),
                ast::Kind::BinaryExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_binary_expression(node))
                }
                ast::Kind::PropertyAccessExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_property_access_expression(node))
                }
                ast::Kind::ElementAccessExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_element_access_expression(node))
                }
                ast::Kind::CallExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_call_expression(node))
                }
                ast::Kind::NewExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_new_expression(node))
                }
                ast::Kind::ArrayLiteralExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_array_literal_expression(node))
                }
                ast::Kind::ObjectLiteralExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_object_literal_expression(node))
                }
                ast::Kind::PropertyAssignment
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_property_assignment(node))
                }
                ast::Kind::MethodDeclaration
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_method_declaration(node))
                }
                ast::Kind::FunctionExpression
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_function_expression(node))
                }
                ast::Kind::Parameter if node.store_id() == self.factory().store().store_id() => {
                    Some(self.visit_each_child_factory_parameter_declaration(node))
                }
                ast::Kind::Block if node.store_id() == self.factory().store().store_id() => {
                    Some(self.visit_each_child_factory_block(node))
                }
                ast::Kind::ReturnStatement
                    if node.store_id() == self.factory().store().store_id() =>
                {
                    Some(self.visit_each_child_factory_return_statement(node))
                }
                ast::Kind::ImportEqualsDeclaration => {
                    self.visit_import_equals_declaration_with_namespace_rules(node)
                }
                _ => Some(self.generated_visit_each_child(&node)),
            }
        };

        self.pop_scope(saved_scope, saved_first);
        self.pop_node(grandparent_node);
        result
    }

    fn should_elide_import_or_export_in_namespace(&self) -> bool {
        self.current_namespace.is_some()
            && self
                .current_scope
                .is_some_and(|scope| self.store_for(scope).kind(scope) != ast::Kind::Block)
    }

    fn visit_identifier(&mut self, node: ast::Node) -> ast::Node {
        let Some(parent) = self.parent_node else {
            return node;
        };
        if is_identifier_reference(self.store_for(node), &node, parent) {
            return self.visit_expression_identifier(node);
        }
        node
    }

    fn visit_expression_identifier(&mut self, node: ast::Node) -> ast::Node {
        if (self.current_enum.is_some() || self.current_namespace.is_some())
            && !is_generated_identifier(self.emit_context, &node)
            && !is_local_name(self.emit_context, &node)
        {
            let location = self.emit_context.most_original(&node);
            if let Some(container) = self
                .facts
                .referenced_export_container(self.store_for(location), location)
                .filter(|container| {
                    ast::is_enum_declaration(self.store_for(*container), *container)
                        || ast::is_module_declaration(self.store_for(*container), *container)
                })
            {
                let container_name = self.get_namespace_container_name(container);
                let member_name = self.clone_node_from_source(node);
                self.emit_context.mark_emit_node(
                    &member_name,
                    printer::EF_NO_COMMENTS | printer::EF_NO_SOURCE_MAP,
                );
                let source = self.source;
                let expression = self.printer_factory().get_namespace_member_name(
                    source,
                    &container_name,
                    &member_name,
                    printer::NameOptions {
                        allow_source_maps: true,
                        ..Default::default()
                    },
                );
                self.emit_context
                    .assign_comment_and_source_map_ranges(&expression, &node);
                return expression;
            }
        }
        node
    }

    fn visit_namespace_export_initializer_identifier(&mut self, node: ast::Node) -> ast::Node {
        if self.current_namespace.is_none() || is_generated_identifier(self.emit_context, &node) {
            return self.preserve_node(node);
        }
        let location = self.emit_context.most_original(&node);
        if let Some(container) = self
            .facts
            .referenced_export_container(self.store_for(location), location)
            .filter(|container| ast::is_module_declaration(self.store_for(*container), *container))
        {
            let container_name = self.get_namespace_container_name(container);
            let member_name = self.clone_node_from_source(node);
            self.emit_context.mark_emit_node(
                &member_name,
                printer::EF_NO_COMMENTS | printer::EF_NO_SOURCE_MAP,
            );
            let source = self.source;
            let expression = self.printer_factory().get_namespace_member_name(
                source,
                &container_name,
                &member_name,
                printer::NameOptions {
                    allow_source_maps: true,
                    ..Default::default()
                },
            );
            self.emit_context
                .assign_comment_and_source_map_ranges(&expression, &node);
            return expression;
        }
        self.preserve_node(node)
    }

    fn visit_heritage_clause(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (token, types_loc, types_range, type_nodes) = {
            let source = self.store_for(node);
            let token = source
                .token(node)
                .expect("heritage clause should have token");
            let source_types = source
                .types(node)
                .expect("heritage clause should have types");
            (
                token,
                source_types.loc(),
                source_types.range(),
                source_types.iter().collect::<Vec<_>>(),
            )
        };
        if token == ast::Kind::ImplementsKeyword {
            return None;
        }
        let mut visited = Vec::with_capacity(type_nodes.len());
        for ty in type_nodes {
            if let Some(ty) = self.visit(ty) {
                visited.push(self.preserve_node(ty));
            }
        }
        let types = self
            .factory()
            .new_node_list(types_loc, types_range, visited);
        Some(self.update_heritage_clause(node, token, types))
    }

    fn visit_expression_with_type_arguments(&mut self, node: ast::Node) -> ast::Node {
        let expression = self
            .store_for(node)
            .expression(node)
            .expect("expression with type arguments should have expression");
        let expression = self.visit(expression).unwrap_or(expression);
        let expression = self.preserve_node(expression);
        self.update_expression_with_type_arguments(node, expression)
    }

    fn visit_module_block(&mut self, node: ast::Node) -> ast::Node {
        let (statements_loc, statements_range, statement_nodes) = {
            let source_statements = self
                .store_for(node)
                .statements(node)
                .expect("module block should have statements");
            (
                source_statements.loc(),
                source_statements.range(),
                source_statements.iter().collect::<Vec<_>>(),
            )
        };
        let statements = self.visit_statement_list(&statement_nodes);
        let statement_list =
            self.factory()
                .new_node_list(statements_loc, statements_range, statements);
        self.update_module_block(node, statement_list)
    }

    fn visit_enum_declaration(&mut self, node: ast::Node) -> ast::Node {
        if !self.should_emit_enum_declaration(node) {
            return self.emit_context.new_not_emitted_statement(&node);
        }

        let mut statements = Vec::new();
        let (updated_statements, var_added) = self.add_var_for_declaration(statements, node);
        statements = updated_statements;

        let mut emit_flags = printer::EF_NONE;
        if var_added
            && (self.compiler_options.get_emit_module_kind() != core::ModuleKind::System
                || !self.current_scope_is_current_source_file())
        {
            emit_flags |= printer::EF_NO_LEADING_COMMENTS;
        }

        let export_reference = self.get_export_qualified_reference_to_declaration(node);
        let assigned_export_reference = self.get_export_qualified_reference_to_declaration(node);
        let empty_properties = self.new_synth_node_list(Vec::<ast::Node>::new());
        let empty_object = self
            .factory()
            .new_object_literal_expression(empty_properties, false);
        let assignment = self
            .printer_factory()
            .new_assignment_expression(assigned_export_reference, empty_object);
        let enum_arg = self
            .printer_factory()
            .new_logical_or_expression(export_reference, assignment);

        let enum_arg = if self.is_export_of_namespace(node) {
            let local_name = self.get_local_name_ex(
                node,
                printer::AssignedNameOptions {
                    allow_source_maps: true,
                    ..Default::default()
                },
            );
            self.printer_factory()
                .new_assignment_expression(local_name, enum_arg)
        } else {
            enum_arg
        };

        let enum_param_name = self.get_namespace_container_name(node);
        if let Some(name) = self.store_for(node).name(node) {
            self.emit_context
                .set_source_map_range(&enum_param_name, self.store_for(name).loc(name));
        }

        let enum_param =
            self.factory()
                .new_parameter_declaration(None, None, enum_param_name, None, None, None);
        let enum_container_name = self.get_namespace_container_name(node);
        let enum_body = self.transform_enum_body(node, enum_container_name);
        let enum_parameters = self.new_synth_node_list(vec![enum_param]);
        let enum_func = self.factory().new_function_expression(
            None,
            None,
            None,
            None,
            enum_parameters,
            None,
            None,
            enum_body,
        );
        let enum_func = self.factory().new_parenthesized_expression(enum_func);
        let enum_arguments = self.new_synth_node_list(vec![enum_arg]);
        let enum_call = self.factory().new_call_expression(
            enum_func,
            None,
            None,
            enum_arguments,
            ast::NodeFlags::NONE,
        );
        let enum_statement = self.factory().new_expression_statement(enum_call);
        self.emit_context.set_original(&enum_statement, &node);
        self.emit_context
            .assign_comment_and_source_map_ranges(&enum_statement, &node);
        self.emit_context
            .mark_emit_node(&enum_statement, emit_flags);
        statements.push(enum_statement);
        self.new_syntax_list_from_vec(statements)
    }

    fn transform_enum_body(&mut self, node: ast::Node, local_name: ast::Node) -> ast::Node {
        let saved_current_enum = self.current_enum;
        let saved_current_namespace_container_name = self.current_namespace_container_name;
        self.current_enum = Some(node);
        self.current_namespace_container_name = Some(local_name);

        let source_members = self
            .store_for(node)
            .source_members(node)
            .expect("enum declaration should have members");
        let source_member_view = source_members;
        let members_loc = source_member_view.loc();
        let source_member_nodes = source_member_view.iter().collect::<Vec<_>>();
        let mut visited_members = Vec::with_capacity(source_member_nodes.len());
        for member in &source_member_nodes {
            if let Some(member) = self.visit(*member) {
                visited_members.push(self.preserve_node(member));
            }
        }

        let mut statements = Vec::new();
        for (index, member) in visited_members.iter().enumerate() {
            self.transform_enum_member(&mut statements, node, source_member_nodes[index], *member);
        }

        let statement_list = self
            .factory()
            .new_node_list(members_loc, members_loc, statements);
        self.current_namespace_container_name = saved_current_namespace_container_name;
        self.current_enum = saved_current_enum;
        self.factory().new_block(statement_list, true)
    }

    fn transform_enum_member(
        &mut self,
        statements: &mut Vec<ast::Node>,
        enum_node: ast::Node,
        source_member: ast::Node,
        member: ast::Node,
    ) {
        let saved_parent = self.parent_node;
        let saved_current = self.current_node;
        self.parent_node = self.current_node;
        self.current_node = Some(source_member);

        let mut expression = self.store().initializer(member);
        let mut use_explicit_reverse_mapping = false;
        let source_member_store = self.store_for(source_member);
        let result = self
            .facts
            .enum_member_value(source_member_store, source_member);
        match result.value {
            evaluator::Value::Number(value) => {
                expression = Some(constant_expression_from_number(value.0, self.factory()));
                use_explicit_reverse_mapping = true;
            }
            evaluator::Value::String(value) => {
                expression = Some(constant_expression_from_string(&value, self.factory()));
            }
            _ => {
                if expression.is_none() {
                    expression = Some(self.printer_factory().new_void_zero_expression());
                }
                use_explicit_reverse_mapping = !result.is_syntactically_string;
            }
        }

        let assignment_target = self.get_enum_qualified_element(enum_node, source_member);
        let mut expression = self.printer_factory().new_assignment_expression(
            assignment_target,
            expression.expect("enum value expression"),
        );

        if use_explicit_reverse_mapping {
            let container_name = self.get_namespace_container_name(enum_node);
            let reverse_target = self.factory().new_element_access_expression(
                container_name,
                None,
                expression,
                ast::NodeFlags::NONE,
            );
            let member_name = self.get_expression_for_property_name(source_member);
            expression = self
                .printer_factory()
                .new_assignment_expression(reverse_target, member_name);
        }

        let member_statement = self.factory().new_expression_statement(expression);
        self.emit_context
            .assign_comment_and_source_map_ranges(&expression, &source_member);
        self.emit_context
            .assign_comment_and_source_map_ranges(&member_statement, &source_member);
        statements.push(member_statement);

        self.current_node = saved_current;
        self.parent_node = saved_parent;
    }

    fn get_expression_for_property_name(&mut self, member: ast::Node) -> ast::Node {
        let (name, kind, computed_expression, literal_text) = {
            let member_source = self.store_for(member);
            let name = member_source
                .name(member)
                .expect("enum member should have a name");
            let name_source = self.store_for(name);
            (
                name,
                name_source.kind(name),
                name_source.expression(name),
                if matches!(
                    name_source.kind(name),
                    ast::Kind::StringLiteral | ast::Kind::NumericLiteral
                ) {
                    Some(name_source.text(name))
                } else {
                    None
                },
            )
        };
        match kind {
            ast::Kind::PrivateIdentifier => self.factory().new_identifier(""),
            ast::Kind::ComputedPropertyName => {
                let expression =
                    computed_expression.expect("computed property name should have expression");
                self.visit(expression)
                    .map(|expression| self.preserve_node(expression))
                    .unwrap_or_else(|| self.factory().new_identifier(""))
            }
            ast::Kind::Identifier => self.new_string_literal_from_node(name),
            ast::Kind::StringLiteral => self.factory().new_string_literal(
                literal_text.expect("literal should have text"),
                ast::TokenFlags::NONE,
            ),
            ast::Kind::NumericLiteral => self.factory().new_numeric_literal(
                literal_text.expect("literal should have text"),
                ast::TokenFlags::NONE,
            ),
            _ => self.preserve_node(name),
        }
    }

    fn get_enum_qualified_element(&mut self, enum_node: ast::Node, member: ast::Node) -> ast::Node {
        let container_name = self.get_namespace_container_name(enum_node);
        let property_name = self.get_expression_for_property_name(member);
        let qualified_name = self.factory().new_element_access_expression(
            container_name,
            None,
            property_name,
            ast::NodeFlags::NONE,
        );
        self.emit_context
            .assign_comment_and_source_map_ranges(&qualified_name, &property_name);
        self.emit_context.mark_emit_node(
            &qualified_name,
            printer::EF_NO_COMMENTS
                | printer::EF_NO_NESTED_COMMENTS
                | printer::EF_NO_SOURCE_MAP
                | printer::EF_NO_NESTED_SOURCE_MAPS,
        );
        qualified_name
    }

    fn visit_module_declaration(&mut self, node: ast::Node) -> ast::Node {
        if !self.should_emit_module_declaration(node) {
            return self.emit_context.new_not_emitted_statement(&node);
        }

        let mut statements = Vec::new();
        let (updated_statements, var_added) = self.add_var_for_declaration(statements, node);
        statements = updated_statements;

        let mut emit_flags = printer::EF_NONE;
        if var_added
            && (self.compiler_options.get_emit_module_kind() != core::ModuleKind::System
                || !self.current_scope_is_current_source_file())
        {
            emit_flags |= printer::EF_NO_LEADING_COMMENTS;
        }

        let export_reference = self.get_export_qualified_reference_to_declaration(node);
        let assigned_export_reference = self.get_export_qualified_reference_to_declaration(node);
        let empty_properties = self.new_synth_node_list(Vec::<ast::Node>::new());
        let empty_object = self
            .factory()
            .new_object_literal_expression(empty_properties, false);
        let assignment = self
            .printer_factory()
            .new_assignment_expression(assigned_export_reference, empty_object);
        let module_arg = self
            .printer_factory()
            .new_logical_or_expression(export_reference, assignment);

        let module_arg = if self.is_export_of_namespace(node) {
            let local_name = self.get_local_name_ex(
                node,
                printer::AssignedNameOptions {
                    allow_source_maps: true,
                    ..Default::default()
                },
            );
            self.printer_factory()
                .new_assignment_expression(local_name, module_arg)
        } else {
            module_arg
        };

        let module_param_name = self.new_generated_name_for_node(node);
        if let Some(name) = self.store_for(node).name(node) {
            self.emit_context
                .set_source_map_range(&module_param_name, self.store_for(name).loc(name));
        }

        let module_param = self.factory().new_parameter_declaration(
            None,
            None,
            module_param_name,
            None,
            None,
            None,
        );
        let namespace_local_name = self.get_namespace_container_name(node);
        let module_body = self.transform_module_body(node, namespace_local_name);
        let module_parameters = self.new_synth_node_list(vec![module_param]);
        let module_func = self.factory().new_function_expression(
            None,
            None,
            None,
            None,
            module_parameters,
            None,
            None,
            module_body,
        );
        let module_func = self.factory().new_parenthesized_expression(module_func);
        let module_arguments = self.new_synth_node_list(vec![module_arg]);
        let module_call = self.factory().new_call_expression(
            module_func,
            None,
            None,
            module_arguments,
            ast::NodeFlags::NONE,
        );
        let module_statement = self.factory().new_expression_statement(module_call);
        self.emit_context.set_original(&module_statement, &node);
        self.emit_context
            .assign_comment_and_source_map_ranges(&module_statement, &node);
        self.emit_context
            .mark_emit_node(&module_statement, emit_flags);
        statements.push(module_statement);
        self.new_syntax_list_from_vec(statements)
    }

    fn transform_module_body(
        &mut self,
        node: ast::Node,
        namespace_local_name: ast::Node,
    ) -> ast::Node {
        let saved_current_namespace_container_name = self.current_namespace_container_name;
        let saved_current_namespace = self.current_namespace;
        let saved_current_scope = self.current_scope;
        let saved_current_scope_first_declarations_of_name =
            self.current_scope_first_declarations_of_name.clone();

        self.current_namespace_container_name = Some(namespace_local_name);
        self.current_namespace = Some(node);
        self.current_scope_first_declarations_of_name = None;

        let mut statements = Vec::new();
        self.emit_context.start_variable_environment();

        let mut statements_location = core::undefined_text_range();
        let mut block_location = core::undefined_text_range();
        if let Some(body) = self.store_for(node).body(node) {
            if self.store_for(body).kind(body) == ast::Kind::ModuleBlock {
                let (saved_body_scope, saved_body_first) = self.push_scope(body);
                let visited_body = self.visit_module_block(body);
                self.pop_scope(saved_body_scope, saved_body_first);
                let statement_list = self
                    .store_for(visited_body)
                    .statements(visited_body)
                    .expect("module block should have statements");
                statements = statement_list.iter().collect();
                statements_location = statement_list.loc();
                block_location = self.store_for(visited_body).loc(visited_body);
            } else {
                statements = self
                    .visit(body)
                    .map(|node| self.flatten_node(node))
                    .unwrap_or_default();
                let innermost_node =
                    get_innermost_module_declaration_from_dotted_module(self.store_for(node), node);
                if let Some(module_block) = self
                    .store_for(innermost_node)
                    .body(innermost_node)
                    .filter(|body| self.store_for(*body).kind(*body) == ast::Kind::ModuleBlock)
                {
                    let statement_list = self
                        .store_for(module_block)
                        .statements(module_block)
                        .expect("module block should have statements");
                    statements_location = statement_list.loc().with_pos(-1);
                }
            }
        }

        self.current_namespace_container_name = saved_current_namespace_container_name;
        self.current_namespace = saved_current_namespace;
        self.current_scope = saved_current_scope;
        self.current_scope_first_declarations_of_name =
            saved_current_scope_first_declarations_of_name;

        let environment = self.emit_context.end_variable_environment();
        let (merged_statements, _) =
            self.emit_context
                .merge_environment(self.source, &statements, &environment);
        statements = merged_statements
            .into_iter()
            .map(|statement| self.preserve_node(statement))
            .collect();
        let statement_list =
            self.factory()
                .new_node_list(statements_location, statements_location, statements);
        let block = self.factory().new_block(statement_list, true);
        self.factory().place_transformed_node(block, block_location);

        if self
            .store_for(node)
            .body(node)
            .is_none_or(|body| self.store_for(body).kind(body) != ast::Kind::ModuleBlock)
        {
            self.emit_context
                .mark_emit_node(&block, printer::EF_NO_COMMENTS);
        }
        block
    }

    fn extract_allowed_modifiers(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
        allowed: ast::ModifierFlags,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let source = modifiers.store();
        let modifier_list = modifiers;
        let modifier_nodes = modifier_list.nodes();
        let filtered: Vec<_> = modifier_nodes
            .iter()
            .filter(|modifier| {
                modifiervisitor::modifier_is_allowed(source.kind(*modifier), allowed)
            })
            .map(|modifier| self.preserve_node(modifier))
            .collect();
        Some(self.factory().new_modifier_list(
            modifier_nodes.loc(),
            modifier_nodes.range(),
            filtered,
            modifier_list.modifier_flags(),
        ))
    }

    fn visit_import_equals_declaration(&mut self, node: ast::Node) -> ast::Node {
        let (module_reference, module_reference_kind, name, loc, allowed_modifiers) = {
            let source = self.store_for(node);
            let module_reference = source
                .module_reference(node)
                .expect("import equals declaration should have module reference");
            let allowed_modifiers = source.source_modifiers(node).map(|modifiers| {
                let modifier_nodes = modifiers.nodes();
                let filtered = modifier_nodes
                    .iter()
                    .filter(|modifier| {
                        modifiervisitor::modifier_is_allowed(
                            source.kind(*modifier),
                            ast::ModifierFlags::EXPORT,
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    modifier_nodes.loc(),
                    modifier_nodes.range(),
                    modifiers.modifier_flags(),
                    filtered,
                )
            });
            (
                module_reference,
                source.kind(module_reference),
                source
                    .name(node)
                    .expect("import equals declaration should have name"),
                source.loc(node),
                allowed_modifiers,
            )
        };
        if module_reference_kind == ast::Kind::ExternalModuleReference {
            return self.preserve_node(node);
        }

        let module_reference = self.create_expression_from_entity_name(module_reference);
        self.emit_context.mark_emit_node(
            &module_reference,
            printer::EF_NO_COMMENTS | printer::EF_NO_NESTED_COMMENTS,
        );

        if !self.is_export_of_namespace(node) {
            let name = self.preserve_node(name);
            let var_decl =
                self.factory()
                    .new_variable_declaration(name, None, None, Some(module_reference));
            self.emit_context.set_original(&var_decl, &node);
            let var_list = self.new_synth_node_list(vec![var_decl]);
            let var_list = self
                .factory()
                .new_variable_declaration_list(var_list, ast::NodeFlags::NONE);
            let var_modifiers =
                allowed_modifiers.and_then(|(loc, range, modifier_flags, modifiers)| {
                    let filtered = modifiers
                        .into_iter()
                        .map(|modifier| self.preserve_node(modifier))
                        .collect::<Vec<_>>();
                    if filtered.is_empty() {
                        None
                    } else {
                        Some(
                            self.factory()
                                .new_modifier_list(loc, range, filtered, modifier_flags),
                        )
                    }
                });
            let var_statement = self
                .factory()
                .new_variable_statement(var_modifiers, var_list);
            self.emit_context.set_original(&var_statement, &node);
            self.emit_context
                .assign_comment_and_source_map_ranges(&var_statement, &node);
            return var_statement;
        }

        let statement = self.create_export_statement(name, module_reference, loc, loc, node);
        self.factory().place_transformed_node(statement, loc);
        statement
    }

    fn visit_import_equals_declaration_with_namespace_rules(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let module_reference = self
            .store_for(node)
            .module_reference(node)
            .expect("import equals declaration should have module reference");
        let module_reference_kind = self.store_for(module_reference).kind(module_reference);
        if self.current_namespace.is_some()
            && self
                .current_scope
                .as_ref()
                .is_some_and(|scope| self.store_for(*scope).kind(*scope) != ast::Kind::Block)
            && module_reference_kind == ast::Kind::ExternalModuleReference
        {
            // do not emit ES6 imports and exports since they are illegal inside a namespace
            None
        } else if self.current_namespace.is_some()
            && self
                .current_scope
                .as_ref()
                .is_some_and(|scope| self.store_for(*scope).kind(*scope) == ast::Kind::Block)
            && module_reference_kind != ast::Kind::ExternalModuleReference
        {
            // inside a block within a namespace, elide internal import aliases
            None
        } else {
            Some(self.visit_import_equals_declaration(node))
        }
    }

    fn visit_function_declaration(&mut self, node: ast::Node) -> ast::Node {
        if self.is_export_of_namespace(node) {
            let (modifiers_input, asterisk_token, name, body, parameters_input) = {
                let source = self.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.asterisk_token(node),
                    source.name(node),
                    source.body(node),
                    source
                        .source_parameters(node)
                        .map(ast::SourceNodeListInput::from_source),
                )
            };
            let modifiers =
                self.extract_modifiers_input(modifiers_input, !ast::ModifierFlags::EXPORT);
            let asterisk_token = self.preserve_optional_node(asterisk_token);
            let name = self.visit_node(name);
            let body = self.visit_node(body);
            let parameters = self
                .visit_nodes_input(parameters_input)
                .expect("function parameters must exist");
            let updated = self.update_function_declaration(
                node,
                modifiers,
                asterisk_token,
                name,
                parameters,
                body,
            );
            if let Some(export) = self.create_export_statement_for_declaration(node) {
                return self.new_syntax_list_from_vec(vec![updated, export]);
            }
            return updated;
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_class_declaration(&mut self, node: ast::Node) -> ast::Node {
        let (member_list, class_name, heritage_clauses_input, modifiers_input) = {
            let source = self.store_for(node);
            (
                ast::SourceNodeListInput::from_source(
                    source
                        .source_members(node)
                        .expect("class declaration should have members"),
                ),
                source.name(node),
                source
                    .source_heritage_clauses(node)
                    .map(ast::SourceNodeListInput::from_source),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        };
        let members = self.visit_class_members_input(member_list);
        let mut class_name = self.preserve_optional_node(class_name);
        let heritage_clauses = self.visit_heritage_clause_list_input(heritage_clauses_input);

        if self.is_export_of_namespace(node) {
            let modifiers =
                self.extract_modifiers_input(modifiers_input, !ast::ModifierFlags::EXPORT_DEFAULT);
            if class_name.is_none() {
                class_name = Some(self.new_generated_name_for_node(node));
            }
            let updated = self.update_class_declaration(
                node,
                modifiers,
                class_name,
                heritage_clauses,
                members,
            );
            if let Some(export) = self.create_export_statement_for_declaration(node) {
                return self.new_syntax_list_from_vec(vec![updated, export]);
            }
            return updated;
        }

        let modifiers = self.import_modifiers_input(modifiers_input);
        self.update_class_declaration(node, modifiers, class_name, heritage_clauses, members)
    }

    fn visit_class_expression(&mut self, node: ast::Node) -> ast::Node {
        let (modifiers_input, name, heritage_clauses_input, member_list) = {
            let source = self.store_for(node);
            (
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source.name(node),
                source
                    .source_heritage_clauses(node)
                    .map(ast::SourceNodeListInput::from_source),
                ast::SourceNodeListInput::from_source(
                    source
                        .source_members(node)
                        .expect("class expression should have members"),
                ),
            )
        };
        let modifiers =
            self.extract_modifiers_input(modifiers_input, !ast::ModifierFlags::EXPORT_DEFAULT);
        let name = self.visit_node(name);
        let heritage_clauses = self.visit_heritage_clause_list_input(heritage_clauses_input);
        let members = self.visit_class_members_input(member_list);
        self.update_class_expression(node, modifiers, name, heritage_clauses, members)
    }

    fn visit_heritage_clause_list(
        &mut self,
        clauses: Option<ast::SourceNodeList<'_>>,
    ) -> Option<ast::NodeList> {
        self.visit_heritage_clause_list_input(clauses.map(ast::SourceNodeListInput::from_source))
    }

    fn visit_heritage_clause_list_input(
        &mut self,
        clauses: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let clauses = clauses?;
        let mut visited = Vec::with_capacity(clauses.len());
        for clause in clauses.iter() {
            if let Some(clause) = self.visit(clause) {
                visited.push(self.preserve_node(clause));
            }
        }
        if visited.is_empty() {
            None
        } else {
            Some(
                self.factory()
                    .new_node_list(clauses.loc(), clauses.range(), visited),
            )
        }
    }

    fn visit_class_members(&mut self, member_list: ast::SourceNodeList<'_>) -> ast::NodeList {
        self.visit_class_members_input(ast::SourceNodeListInput::from_source(member_list))
    }

    fn visit_class_members_input(
        &mut self,
        member_list: ast::SourceNodeListInput,
    ) -> ast::NodeList {
        let constructor = member_list
            .iter()
            .find(|member| ast::is_constructor_declaration(self.store_for(*member), *member));
        if self.get_parameter_properties(constructor).is_empty() {
            return self
                .visit_nodes_input(Some(member_list))
                .expect("class members are required");
        }

        let members = self.visit_class_element_list_input(&member_list);
        self.add_parameter_property_declarations_input(Some(&member_list), Some(members))
    }

    fn visit_class_element_list(
        &mut self,
        members: ast::SourceNodeList<'_>,
    ) -> (ast::NodeList, Vec<ast::Node>) {
        self.visit_class_element_list_input(&ast::SourceNodeListInput::from_source(members))
    }

    fn visit_class_element_list_input(
        &mut self,
        source_members: &ast::SourceNodeListInput,
    ) -> (ast::NodeList, Vec<ast::Node>) {
        let mut visited = Vec::with_capacity(source_members.len());
        for member in source_members.iter() {
            if let Some(member) = self.visit(member) {
                let flattened = self.flatten_node(member);
                visited.extend(
                    flattened
                        .into_iter()
                        .map(|member| self.preserve_node(member)),
                );
            }
        }
        let list = self.factory().new_node_list(
            source_members.loc(),
            source_members.range(),
            visited.clone(),
        );
        (list, visited)
    }

    fn add_parameter_property_declarations(
        &mut self,
        original_members: Option<ast::SourceNodeList<'_>>,
        members: Option<(ast::NodeList, Vec<ast::Node>)>,
    ) -> ast::NodeList {
        let original_members = original_members.map(ast::SourceNodeListInput::from_source);
        self.add_parameter_property_declarations_input(original_members.as_ref(), members)
    }

    fn add_parameter_property_declarations_input(
        &mut self,
        original_members: Option<&ast::SourceNodeListInput>,
        members: Option<(ast::NodeList, Vec<ast::Node>)>,
    ) -> ast::NodeList {
        let constructor = original_members.and_then(|members| {
            members
                .iter()
                .find(|member| ast::is_constructor_declaration(self.store_for(*member), *member))
        });
        let parameter_properties = self.get_parameter_properties(constructor);
        if parameter_properties.is_empty() {
            return members.expect("class members are required").0;
        }

        let mut new_members = Vec::new();
        for parameter in parameter_properties {
            let Some(name) = ({
                let source = self.store_for(parameter);
                source
                    .name(parameter)
                    .filter(|name| ast::is_identifier(source, *name))
            }) else {
                continue;
            };
            let name = self.clone_node_from_source(name);
            let property = self
                .factory()
                .new_property_declaration(None, name, None, None, None);
            self.emit_context.set_original(&property, &parameter);
            new_members.push(property);
        }
        if new_members.is_empty() {
            return members.expect("class members are required").0;
        }

        if let Some((_members, member_nodes)) = members {
            let members = original_members.expect("original class members are required");
            new_members.extend(member_nodes);
            self.factory()
                .new_node_list(members.loc(), members.range(), new_members)
        } else {
            self.new_synth_node_list(new_members)
        }
    }

    fn visit_constructor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let (parameters_input, body, modifiers_input) = {
            let source = self.store_for(node);
            (
                ast::SourceNodeListInput::from_source(
                    source
                        .source_parameters(node)
                        .expect("constructor should have parameters"),
                ),
                source.body(node),
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
            )
        };
        let parameters = self.visit_parameter_list_input(parameters_input);
        let body = body.map(|body| self.visit_constructor_body(body, node));
        let modifiers = self.import_modifiers_input(modifiers_input);
        let body = self.preserve_optional_node(body);
        let updated = self.update_constructor_declaration(node, modifiers, parameters, body);
        self.emit_context
            .assign_comment_and_source_map_ranges(&updated, &node);
        updated
    }

    fn visit_parameter_list(&mut self, parameters: ast::SourceNodeList<'_>) -> ast::NodeList {
        self.visit_parameter_list_input(ast::SourceNodeListInput::from_source(parameters))
    }

    fn visit_parameter_list_input(
        &mut self,
        source_parameters: ast::SourceNodeListInput,
    ) -> ast::NodeList {
        let old_flags = self.emit_context.begin_visit_parameters();
        let mut visited = Vec::with_capacity(source_parameters.len());
        let mut changed = false;
        for parameter in source_parameters.iter() {
            let (modifiers_input, dot_dot_dot_token, name, question_token, type_node, initializer) = {
                let source = self.store_for(parameter);
                (
                    source
                        .source_modifiers(parameter)
                        .map(ast::SourceModifierListInput::from_source),
                    source.dot_dot_dot_token(parameter),
                    source.name(parameter),
                    source.question_token(parameter),
                    source.r#type(parameter),
                    source.initializer(parameter),
                )
            };
            let modifiers = self.strip_parameter_property_modifiers_input(modifiers_input);
            let dot_dot_dot_token = self.preserve_optional_node(dot_dot_dot_token);
            let name = self.preserve_optional_node(name);
            let question_token = self.preserve_optional_node(question_token);
            let type_node = self.preserve_optional_node(type_node);
            let grandparent_node = self.push_node(parameter);
            let initializer = self.visit_node(initializer);
            self.pop_node(grandparent_node);
            let parameter = self.update_parameter_declaration(
                parameter,
                modifiers,
                dot_dot_dot_token,
                name,
                question_token,
                type_node,
                initializer,
            );
            changed = true;
            visited.push(parameter);
        }
        let (visited, _) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        self.factory()
            .new_node_list(source_parameters.loc(), source_parameters.range(), visited)
    }

    fn get_parameter_properties(&self, constructor: Option<ast::Node>) -> Vec<ast::Node> {
        let Some(constructor_node) = constructor else {
            return Vec::new();
        };
        self.store_for(constructor_node)
            .parameters(constructor_node)
            .expect("constructor should have parameters")
            .iter()
            .filter(|parameter| {
                ast::is_parameter_property_declaration(
                    self.store_for(*parameter),
                    *parameter,
                    constructor_node,
                )
            })
            .collect()
    }

    fn visit_constructor_body(&mut self, body: ast::Node, constructor: ast::Node) -> ast::Node {
        let parameter_properties = self.get_parameter_properties(Some(constructor));
        if parameter_properties.is_empty() {
            return self
                .visit_function_body(Some(body))
                .unwrap_or_else(|| self.preserve_node(body));
        }

        let grandparent_node = self.push_node(body);
        let (saved_scope, saved_first) = self.push_scope(body);

        self.emit_context.start_variable_environment();
        let source_statements = self
            .store_for(body)
            .statements(body)
            .expect("block should have statements");
        let statements_loc = source_statements.loc();
        let statements_range = source_statements.range();
        let original_statements: Vec<_> = source_statements.iter().collect();
        let (prologue, rest) = self.split_standard_prologue(&original_statements);

        // Transform parameters into property assignments. Transforms this:
        //
        //  constructor (public x, public y) {
        //  }
        //
        // Into this:
        //
        //  constructor (x, y) {
        //      this.x = x;
        //      this.y = y;
        //  }
        //
        let assignments = self.create_parameter_property_assignments(&parameter_properties);
        let mut statements = prologue
            .iter()
            .copied()
            .map(|statement| self.preserve_node(statement))
            .collect::<Vec<_>>();

        if let Some(super_index) = self.find_super_statement_index(rest) {
            statements.extend(self.visit_statement_list(&rest[..=super_index]));
            statements.extend(assignments);
            statements.extend(self.visit_statement_list(&rest[super_index + 1..]));
        } else {
            statements.extend(assignments);
            statements.extend(self.visit_statement_list(rest));
        }
        let statements = self
            .emit_context
            .end_and_merge_variable_environment(self.source, &statements);
        // Close the parameter environment opened by VisitParameters. Its declarations
        // have already been represented by the synthetic parameter-property assignments.
        let _ = self.emit_context.end_variable_environment();

        let statement_list =
            self.factory()
                .new_node_list(statements_loc, statements_range, statements);
        let updated = self.factory().new_block(statement_list, true);
        let body_loc = self.store_for(body).loc(body);
        self.factory().place_transformed_node(updated, body_loc);
        self.emit_context.set_original(&updated, &body);
        self.pop_scope(saved_scope, saved_first);
        self.pop_node(grandparent_node);
        updated
    }

    fn split_standard_prologue<'a>(
        &self,
        statements: &'a [ast::Node],
    ) -> (&'a [ast::Node], &'a [ast::Node]) {
        for (i, statement) in statements.iter().enumerate() {
            if !ast::is_prologue_directive(self.store_for(*statement), *statement) {
                return (&statements[..i], &statements[i..]);
            }
        }
        (statements, &[])
    }

    fn create_parameter_property_assignments(
        &mut self,
        parameter_properties: &[ast::Node],
    ) -> Vec<ast::Node> {
        let mut statements = Vec::new();
        for parameter in parameter_properties {
            let Some(name) = self
                .store_for(*parameter)
                .name(*parameter)
                .filter(|name| ast::is_identifier(self.store_for(*name), *name))
            else {
                continue;
            };
            let name_parent = self.store_for(name).parent(name);
            let property_name = self.clone_node_from_source(name);
            self.factory()
                .link_emit_synthetic_parent(property_name, name_parent);
            self.emit_context.mark_emit_node(
                &property_name,
                printer::EF_NO_COMMENTS | printer::EF_NO_SOURCE_MAP,
            );
            let local_name = self.clone_node_from_source(name);
            self.factory()
                .link_emit_synthetic_parent(local_name, name_parent);
            self.emit_context
                .mark_emit_node(&local_name, printer::EF_NO_COMMENTS);
            let this_expression = self.printer_factory().new_this_expression();
            let access = self.factory().new_property_access_expression(
                this_expression,
                None,
                property_name,
                ast::NodeFlags::NONE,
            );
            let assignment = self
                .printer_factory()
                .new_assignment_expression(access, local_name);
            let statement = self.factory().new_expression_statement(assignment);
            self.emit_context.set_original(&statement, parameter);
            self.emit_context
                .mark_emit_node(&statement, printer::EF_START_ON_NEW_LINE);
            statements.push(statement);
        }
        statements
    }

    fn visit_shorthand_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        let (
            name,
            object_assignment_initializer,
            equals_token,
            modifier_nodes,
            postfix_token,
            node_loc,
            _is_factory_node,
        ) = {
            let source = self.store_for(node);
            (
                source
                    .name(node)
                    .expect("shorthand property assignment should have name"),
                source.object_assignment_initializer(node),
                source.equals_token(node),
                source.source_modifiers(node).map(|modifiers| {
                    let nodes = modifiers.nodes();
                    (
                        nodes.nodes(),
                        nodes.loc(),
                        nodes.range(),
                        modifiers.modifier_flags(),
                    )
                }),
                source.postfix_token(node),
                source.loc(node),
                source.store_id() == self.store().store_id(),
            )
        };
        let exported_or_imported_name = self.visit_expression_identifier(name);
        if exported_or_imported_name != name {
            let mut expression = exported_or_imported_name;
            if let Some(initializer) = object_assignment_initializer {
                let equals_token = equals_token
                    .map(|token| self.preserve_node(token))
                    .unwrap_or_else(|| self.factory().new_token(ast::Kind::EqualsToken));
                let initializer = self
                    .visit_node(Some(initializer))
                    .expect("initialized variable should visit to an expression");
                expression = self.factory().new_binary_expression(
                    None,
                    expression,
                    None,
                    equals_token,
                    initializer,
                );
            }

            let name = self.preserve_node(name);
            let updated = self
                .factory()
                .new_property_assignment(None, name, None, None, expression);
            self.factory().place_transformed_node(updated, node_loc);
            self.emit_context.set_original(&updated, &node);
            self.emit_context
                .assign_comment_and_source_map_ranges(&updated, &node);
            return updated;
        }

        let modifiers = self.visit_factory_modifier_list(modifier_nodes);
        let name = Some(self.preserve_node(name));
        let postfix_token = self.preserve_optional_node(postfix_token);
        let equals_token = self.preserve_optional_node(equals_token);
        let initializer = self.visit_node(object_assignment_initializer);
        self.update_shorthand_property_assignment(
            node,
            modifiers,
            name,
            postfix_token,
            equals_token,
            initializer,
        )
    }

    fn visit_variable_statement(&mut self, node: ast::Node) -> Option<ast::Node> {
        if !self.is_export_of_namespace(node) {
            return Some(self.generated_visit_each_child(&node));
        }
        let declaration_list = self.store_for(node).declaration_list(node)?;
        let declarations: Vec<_> = self
            .store_for(declaration_list)
            .declarations(declaration_list)
            .expect("variable declaration list should have declarations")
            .iter()
            .collect();
        let mut expressions = Vec::new();
        for declaration in declarations {
            let Some(_initializer) = self.store_for(declaration).initializer(declaration) else {
                continue;
            };
            let Some(name) = self.store_for(declaration).name(declaration) else {
                continue;
            };
            if ast::is_binding_pattern(self.store_for(name), name) {
                let namespace = self
                    .current_namespace
                    .expect("namespace export requires current namespace");
                let namespace_name = self.get_namespace_container_name(namespace);
                let source = self.source;
                let mut create_namespace_export_expression =
                    |emit_context: &mut printer::EmitContext,
                     export_name: ast::Node,
                     export_value: ast::Node,
                     location: core::TextRange| {
                        let member_name = emit_context.factory.get_namespace_member_name(
                            source,
                            &namespace_name,
                            &export_name,
                            printer::NameOptions {
                                allow_source_maps: true,
                                ..Default::default()
                            },
                        );
                        let expression = emit_context
                            .factory
                            .new_assignment_expression(member_name, export_value);
                        emit_context
                            .factory
                            .node_factory
                            .place_emit_synthetic_node(expression, location);
                        expression
                    };
                expressions.push(crate::destructuring::flatten_destructuring_assignment(
                    source,
                    self.emit_context,
                    declaration,
                    false,
                    crate::destructuring::FlattenLevel::All,
                    Some(&mut create_namespace_export_expression),
                ));
            } else if ast::is_identifier(self.store_for(name), name) {
                if let Some(expression) =
                    crate::utilities::convert_variable_declaration_to_assignment_expression(
                        self.emit_context,
                        self.source,
                        declaration,
                    )
                {
                    expressions.push(expression);
                }
            }
        }
        if expressions.is_empty() {
            return None;
        }
        let expression = self
            .printer_factory()
            .inline_expressions(&expressions)
            .expect("exported variable expressions should not be empty");
        let statement = self.factory().new_expression_statement(expression);
        self.emit_context.set_original(&statement, &node);
        self.emit_context
            .assign_comment_and_source_map_ranges(&statement, &node);

        // re-visit as the new node
        let saved_current = self.current_node;
        self.current_node = Some(statement);
        let statement = self.visit_each_child_factory_expression_statement(statement);
        self.current_node = saved_current;
        Some(statement)
    }

    fn visit_each_child_factory_expression_statement(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let expression = self.store_for(node).expression(node);
        let expression = self.visit_node(expression);
        self.factory().update_expression_statement(node, expression)
    }

    fn visit_each_child_factory_binary_expression(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (left, type_node, operator_token, right) = {
            let store = self.store_for(node);
            (
                store.left(node),
                store.r#type(node),
                store.operator_token(node),
                store.right(node),
            )
        };
        let left = self.visit_node(left);
        let type_node = self.visit_node(type_node);
        let operator_token = self.visit_node(operator_token);
        let right = self.visit_node(right);
        self.factory().update_binary_expression(
            node,
            None::<ast::ModifierList>,
            left,
            type_node,
            operator_token,
            right,
        )
    }

    fn visit_each_child_factory_property_access_expression(
        &mut self,
        node: ast::Node,
    ) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (expression, question_dot_token, name, flags) = {
            let store = self.store_for(node);
            (
                store.expression(node),
                store.question_dot_token(node),
                store.name(node),
                store.flags(node),
            )
        };
        let expression = self.visit_node(expression);
        let question_dot_token = self.visit_node(question_dot_token);
        let name = self.visit_node(name);
        self.factory().update_property_access_expression(
            node,
            expression,
            question_dot_token,
            name,
            flags,
        )
    }

    fn visit_each_child_factory_element_access_expression(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (expression, question_dot_token, argument_expression, flags) = {
            let store = self.store_for(node);
            (
                store.expression(node),
                store.question_dot_token(node),
                store.argument_expression(node),
                store.flags(node),
            )
        };
        let expression = self.visit_node(expression);
        let question_dot_token = self.visit_node(question_dot_token);
        let argument_expression = self.visit_node(argument_expression);
        self.factory().update_element_access_expression(
            node,
            expression,
            question_dot_token,
            argument_expression,
            flags,
        )
    }

    fn visit_each_child_factory_call_expression(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (expression, question_dot_token, type_argument_nodes, argument_nodes, flags) = {
            let store = self.store_for(node);
            let type_arguments = store.type_arguments(node).map(|type_arguments| {
                (
                    type_arguments.nodes(),
                    type_arguments.loc(),
                    type_arguments.range(),
                    type_arguments.has_trailing_comma(),
                )
            });
            let arguments = store
                .arguments(node)
                .expect("call expression should have arguments");
            let arguments = (
                arguments.nodes(),
                arguments.loc(),
                arguments.range(),
                arguments.has_trailing_comma(),
            );
            (
                store.expression(node),
                store.question_dot_token(node),
                type_arguments,
                arguments,
                store.flags(node),
            )
        };
        let expression = self.visit_node(expression);
        let question_dot_token = self.visit_node(question_dot_token);
        let type_arguments =
            type_argument_nodes.map(|(source_nodes, loc, range, has_trailing_comma)| {
                self.visit_factory_node_list(loc, range, has_trailing_comma, source_nodes)
            });
        let arguments = {
            let (source_nodes, loc, range, has_trailing_comma) = argument_nodes;
            self.visit_factory_node_list(loc, range, has_trailing_comma, source_nodes)
        };
        self.factory().update_call_expression(
            node,
            expression,
            question_dot_token,
            type_arguments,
            arguments,
            flags,
        )
    }

    fn visit_each_child_factory_new_expression(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (expression, type_argument_nodes, argument_nodes) = {
            let store = self.store_for(node);
            let type_arguments = store.type_arguments(node).map(|type_arguments| {
                (
                    type_arguments.nodes(),
                    type_arguments.loc(),
                    type_arguments.range(),
                    type_arguments.has_trailing_comma(),
                )
            });
            let arguments = store.arguments(node).map(|arguments| {
                (
                    arguments.nodes(),
                    arguments.loc(),
                    arguments.range(),
                    arguments.has_trailing_comma(),
                )
            });
            (store.expression(node), type_arguments, arguments)
        };
        let expression = self.visit_node(expression);
        let type_arguments =
            type_argument_nodes.map(|(source_nodes, loc, range, has_trailing_comma)| {
                self.visit_factory_node_list(loc, range, has_trailing_comma, source_nodes)
            });
        let arguments = argument_nodes.map(|(source_nodes, loc, range, has_trailing_comma)| {
            self.visit_factory_node_list(loc, range, has_trailing_comma, source_nodes)
        });
        self.factory()
            .update_new_expression(node, expression, type_arguments, arguments)
    }

    fn visit_each_child_factory_array_literal_expression(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (element_nodes, elements_loc, elements_range, has_trailing_comma, multi_line) = {
            let store = self.store_for(node);
            let elements = store
                .elements(node)
                .expect("array literal should have elements");
            (
                elements.nodes(),
                elements.loc(),
                elements.range(),
                elements.has_trailing_comma(),
                store.multi_line(node).unwrap_or(false),
            )
        };
        let elements = self.visit_factory_node_list(
            elements_loc,
            elements_range,
            has_trailing_comma,
            element_nodes,
        );
        self.factory()
            .update_array_literal_expression(node, elements, multi_line)
    }

    fn visit_each_child_factory_object_literal_expression(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (property_nodes, properties_loc, properties_range, has_trailing_comma, multi_line) = {
            let store = self.store_for(node);
            let properties = store
                .properties(node)
                .expect("object literal should have properties");
            let property_nodes = properties.nodes();
            let properties_loc = properties.loc();
            let properties_range = properties.range();
            let has_trailing_comma = properties.has_trailing_comma();
            let multi_line = store.multi_line(node).unwrap_or(false);
            (
                property_nodes,
                properties_loc,
                properties_range,
                has_trailing_comma,
                multi_line,
            )
        };
        let properties = self.visit_factory_node_list(
            properties_loc,
            properties_range,
            has_trailing_comma,
            property_nodes,
        );
        self.factory()
            .update_object_literal_expression(node, properties, multi_line)
    }

    fn visit_each_child_factory_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (modifier_nodes, name, postfix_token, type_node, initializer) = {
            let store = self.store_for(node);
            let modifiers = store.modifiers(node).map(|modifiers| {
                (
                    modifiers.nodes().nodes(),
                    modifiers.loc(),
                    modifiers.range(),
                    modifiers.modifier_flags(),
                )
            });
            (
                modifiers,
                store.name(node),
                store.postfix_token(node),
                store.r#type(node),
                store.initializer(node),
            )
        };
        let modifiers = self.visit_factory_modifier_list(modifier_nodes);
        let name = self.visit_node(name);
        let postfix_token = self.visit_node(postfix_token);
        let type_node = self.visit_node(type_node);
        let initializer = self.visit_node(initializer);
        self.factory().update_property_assignment(
            node,
            modifiers,
            name,
            postfix_token,
            type_node,
            initializer,
        )
    }

    fn visit_each_child_factory_function_expression(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (
            modifier_nodes,
            asterisk_token,
            name,
            type_parameter_nodes,
            parameter_nodes,
            type_node,
            full_signature,
            body,
        ) = {
            let store = self.store_for(node);
            let modifiers = store.modifiers(node).map(|modifiers| {
                (
                    modifiers.nodes().nodes(),
                    modifiers.loc(),
                    modifiers.range(),
                    modifiers.modifier_flags(),
                )
            });
            let type_parameters = store.type_parameters(node).map(|type_parameters| {
                (
                    type_parameters.nodes(),
                    type_parameters.loc(),
                    type_parameters.range(),
                    type_parameters.has_trailing_comma(),
                )
            });
            let parameters = store
                .parameters(node)
                .expect("function expression should have parameters");
            let parameters = (
                parameters.nodes(),
                parameters.loc(),
                parameters.range(),
                parameters.has_trailing_comma(),
            );
            (
                modifiers,
                store.asterisk_token(node),
                store.name(node),
                type_parameters,
                parameters,
                store.r#type(node),
                store.full_signature(node),
                store.body(node),
            )
        };
        let modifiers = self.visit_factory_modifier_list(modifier_nodes);
        let asterisk_token = self.visit_node(asterisk_token);
        let name = self.visit_node(name);
        let type_parameters =
            type_parameter_nodes.map(|(source_nodes, loc, range, has_trailing_comma)| {
                self.visit_factory_node_list(loc, range, has_trailing_comma, source_nodes)
            });
        let parameters = self.visit_factory_parameters(parameter_nodes);
        let type_node = self.visit_node(type_node);
        let full_signature = self.visit_node(full_signature);
        let body = self.visit_function_body(body);
        self.factory().update_function_expression(
            node,
            modifiers,
            asterisk_token,
            name,
            type_parameters,
            parameters,
            type_node,
            full_signature,
            body,
        )
    }

    fn visit_each_child_factory_method_declaration(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (
            modifier_nodes,
            asterisk_token,
            name,
            postfix_token,
            type_parameter_nodes,
            parameter_nodes,
            type_node,
            full_signature,
            body,
        ) = {
            let store = self.store_for(node);
            let modifiers = store.modifiers(node).map(|modifiers| {
                (
                    modifiers.nodes().nodes(),
                    modifiers.loc(),
                    modifiers.range(),
                    modifiers.modifier_flags(),
                )
            });
            let type_parameters = store.type_parameters(node).map(|type_parameters| {
                (
                    type_parameters.nodes(),
                    type_parameters.loc(),
                    type_parameters.range(),
                    type_parameters.has_trailing_comma(),
                )
            });
            let parameters = store
                .parameters(node)
                .expect("method declaration should have parameters");
            let parameters = (
                parameters.nodes(),
                parameters.loc(),
                parameters.range(),
                parameters.has_trailing_comma(),
            );
            (
                modifiers,
                store.asterisk_token(node),
                store.name(node),
                store.postfix_token(node),
                type_parameters,
                parameters,
                store.r#type(node),
                store.full_signature(node),
                store.body(node),
            )
        };
        let modifiers = self.visit_factory_modifier_list(modifier_nodes);
        let asterisk_token = self.visit_node(asterisk_token);
        let name = self.visit_node(name);
        let postfix_token = self.visit_node(postfix_token);
        let type_parameters =
            type_parameter_nodes.map(|(source_nodes, loc, range, has_trailing_comma)| {
                self.visit_factory_node_list(loc, range, has_trailing_comma, source_nodes)
            });
        let parameters = self.visit_factory_parameters(parameter_nodes);
        let type_node = self.visit_node(type_node);
        let full_signature = self.visit_node(full_signature);
        let body = self.visit_function_body(body);
        self.factory().update_method_declaration(
            node,
            modifiers,
            asterisk_token,
            name,
            postfix_token,
            type_parameters,
            parameters,
            type_node,
            full_signature,
            body,
        )
    }

    fn visit_each_child_factory_parameter_declaration(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (modifier_nodes, dot_dot_dot_token, name, question_token, type_node, initializer) = {
            let store = self.store_for(node);
            let modifiers = store.modifiers(node).map(|modifiers| {
                (
                    modifiers.nodes().nodes(),
                    modifiers.loc(),
                    modifiers.range(),
                    modifiers.modifier_flags(),
                )
            });
            (
                modifiers,
                store.dot_dot_dot_token(node),
                store.name(node),
                store.question_token(node),
                store.r#type(node),
                store.initializer(node),
            )
        };
        let modifiers = self.visit_factory_modifier_list(modifier_nodes);
        let dot_dot_dot_token = self.visit_node(dot_dot_dot_token);
        let name = self.visit_node(name);
        let question_token = self.visit_node(question_token);
        let type_node = self.visit_node(type_node);
        let initializer = self.visit_node(initializer);
        self.factory().update_parameter_declaration(
            node,
            modifiers,
            dot_dot_dot_token,
            name,
            question_token,
            type_node,
            initializer,
        )
    }

    fn visit_each_child_factory_block(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let (statement_nodes, statements_loc, statements_range, has_trailing_comma, multi_line) = {
            let store = self.store_for(node);
            let statements = store
                .statements(node)
                .expect("block should have statements");
            (
                statements.nodes(),
                statements.loc(),
                statements.range(),
                statements.has_trailing_comma(),
                store
                    .multi_line(node)
                    .expect("block should have multi_line"),
            )
        };
        let statements = self.visit_factory_node_list(
            statements_loc,
            statements_range,
            has_trailing_comma,
            statement_nodes,
        );
        self.factory().update_block(node, statements, multi_line)
    }

    fn visit_each_child_factory_return_statement(&mut self, node: ast::Node) -> ast::Node {
        debug_assert_eq!(node.store_id(), self.factory().store().store_id());
        let expression = self.store_for(node).expression(node);
        let expression = self.visit_node(expression);
        self.factory().update_return_statement(node, expression)
    }

    fn visit_factory_node_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        has_trailing_comma: bool,
        source_nodes: Vec<ast::Node>,
    ) -> ast::NodeList {
        let mut visited = Vec::with_capacity(source_nodes.len());
        let mut changed = false;
        for node in source_nodes.iter() {
            let result = self.visit(*node);
            self.append_visited_node(*node, result, &mut visited, &mut changed);
        }
        self.factory_mut().new_node_list_with_trailing_comma(
            loc,
            range,
            visited,
            has_trailing_comma,
        )
    }

    fn visit_factory_parameters(
        &mut self,
        parameters: (Vec<ast::Node>, core::TextRange, core::TextRange, bool),
    ) -> ast::NodeList {
        let (source_nodes, loc, range, has_trailing_comma) = parameters;
        let old_flags = self.emit_context.begin_visit_parameters();
        let mut visited = Vec::with_capacity(source_nodes.len());
        let mut changed = false;
        for node in source_nodes.iter() {
            let result = self.visit(*node);
            self.append_visited_node(*node, result, &mut visited, &mut changed);
        }
        let (visited, _) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        self.factory_mut().new_node_list_with_trailing_comma(
            loc,
            range,
            visited,
            has_trailing_comma,
        )
    }

    fn visit_factory_modifier_list(
        &mut self,
        modifiers: Option<(
            Vec<ast::Node>,
            core::TextRange,
            core::TextRange,
            ast::ModifierFlags,
        )>,
    ) -> Option<ast::ModifierList> {
        let (source_nodes, loc, range, modifier_flags) = modifiers?;
        let mut visited = Vec::with_capacity(source_nodes.len());
        let mut changed = false;
        for node in source_nodes.iter() {
            let result = self.visit(*node);
            self.append_visited_node(*node, result, &mut visited, &mut changed);
        }
        Some(
            self.factory_mut()
                .new_modifier_list(loc, range, visited, modifier_flags),
        )
    }

    fn record_declaration_in_scope(&mut self, node: ast::Node) {
        if self.store_for(node).kind(node) == ast::Kind::VariableStatement {
            if let Some(declaration_list) = self.store_for(node).declaration_list(node) {
                self.record_declaration_in_scope(declaration_list);
            }
            return;
        }
        if self.store_for(node).kind(node) == ast::Kind::VariableDeclarationList {
            let declarations = self
                .store_for(node)
                .source_declarations(node)
                .expect("variable declaration list should have declarations");
            let declarations: Vec<_> = declarations.iter().collect();
            for declaration in &declarations {
                self.record_declaration_in_scope(*declaration);
            }
            return;
        }
        if matches!(
            self.store_for(node).kind(node),
            ast::Kind::ArrayBindingPattern | ast::Kind::ObjectBindingPattern
        ) {
            let elements = self
                .store_for(node)
                .elements(node)
                .expect("binding pattern should have elements");
            let elements: Vec<_> = elements.iter().collect();
            for element in &elements {
                self.record_declaration_in_scope(*element);
            }
            return;
        }
        let Some(name) = self.store_for(node).name(node) else {
            return;
        };
        if ast::is_identifier(self.store_for(name), name) {
            let name_text = self.store_for(name).text(name);
            let node_id = self.store_for(node).get_node_id(node);
            let declarations = self
                .current_scope_first_declarations_of_name
                .get_or_insert_with(HashMap::new);
            declarations.entry(name_text).or_insert(node_id);
        } else if ast::is_binding_pattern(self.store_for(name), name) {
            self.record_declaration_in_scope(name);
        }
    }

    fn is_first_declaration_in_scope(&self, node: ast::Node) -> bool {
        let Some(name) = self.store_for(node).name(node) else {
            return false;
        };
        if !ast::is_identifier(self.store_for(name), name) {
            return false;
        }
        let name_text = self.store_for(name).text(name);
        self.current_scope_first_declarations_of_name
            .as_ref()
            .and_then(|declarations| declarations.get(&name_text))
            .is_some_and(|first| *first == self.store_for(node).get_node_id(node))
    }

    fn is_export_of_namespace(&self, node: ast::Node) -> bool {
        self.current_namespace.is_some()
            && self
                .current_scope
                .as_ref()
                .is_none_or(|scope| self.store_for(*scope).kind(*scope) != ast::Kind::Block)
            && ast::has_syntactic_modifier(self.store_for(node), node, ast::ModifierFlags::EXPORT)
    }

    fn get_namespace_container_name(&mut self, node: ast::Node) -> ast::Node {
        self.new_generated_name_for_node(node)
    }

    fn get_namespace_qualified_property(&mut self, ns: ast::Node, name: ast::Node) -> ast::Node {
        let source = self.source;
        self.emit_context.factory.get_namespace_member_name(
            source,
            &ns,
            &name,
            printer::NameOptions {
                allow_source_maps: true,
                ..Default::default()
            },
        )
    }

    fn get_export_qualified_reference_to_declaration(&mut self, node: ast::Node) -> ast::Node {
        if self.is_export_of_namespace(node) {
            let namespace = self
                .current_namespace
                .expect("namespace export requires current namespace");
            let namespace_name = self.get_namespace_container_name(namespace);
            return self.get_external_module_or_namespace_export_name(
                Some(&namespace_name),
                node,
                false,
                true,
            );
        }
        self.get_declaration_name_ex(
            node,
            printer::NameOptions {
                allow_source_maps: true,
                ..Default::default()
            },
        )
    }

    fn add_var_for_declaration(
        &mut self,
        mut statements: Vec<ast::Node>,
        node: ast::Node,
    ) -> (Vec<ast::Node>, bool) {
        self.record_declaration_in_scope(node);
        if !self.is_first_declaration_in_scope(node) {
            return (statements, false);
        }

        let name = self.get_local_name_ex(
            node,
            printer::AssignedNameOptions {
                allow_source_maps: true,
                ..Default::default()
            },
        );
        let var_decl = self
            .factory()
            .new_variable_declaration(name, None, None, None);
        let var_flags = if self.current_scope_is_current_source_file() {
            ast::NodeFlags::NONE
        } else {
            ast::NodeFlags::LET
        };
        let var_decl_list = self.new_synth_node_list(vec![var_decl]);
        let var_decls = self
            .factory()
            .new_variable_declaration_list(var_decl_list, var_flags);
        let modifier_nodes = self
            .store_for(node)
            .source_modifiers(node)
            .map(|modifiers| {
                (
                    modifiers.loc(),
                    modifiers.range(),
                    modifiers.nodes().nodes(),
                    modifiers.modifier_flags(),
                )
            });
        let inside_namespace = self.current_namespace.is_some();
        let modifiers = self.strip_runtime_modifiers_for_var(modifier_nodes, inside_namespace);
        let var_statement = self.factory().new_variable_statement(modifiers, var_decls);

        self.emit_context.set_original(&var_decl, &node);
        self.emit_context.set_original(&var_statement, &node);
        let node_loc = self.store_for(node).loc(node);
        if ast::is_enum_declaration(self.store_for(node), node) {
            self.emit_context.set_source_map_range(&var_decls, node_loc);
        } else {
            self.emit_context
                .set_source_map_range(&var_statement, node_loc);
        }
        self.emit_context
            .set_comment_range(&var_statement, node_loc);
        self.emit_context
            .mark_emit_node(&var_statement, printer::EF_NO_TRAILING_COMMENTS);
        statements.push(var_statement);
        (statements, true)
    }

    fn should_emit_module_declaration(&mut self, node: ast::Node) -> bool {
        let parse_node = self.emit_context.parse_node(&node);
        if let Some(parse_node) = parse_node
            && parse_node != node
        {
            return ast::is_instantiated_module(
                self.store_for(parse_node),
                parse_node,
                self.compiler_options.should_preserve_const_enums(),
            );
        }
        self.store_for(node).body(node).is_some()
    }

    fn should_emit_enum_declaration(&self, node: ast::Node) -> bool {
        !ast::is_enum_const(self.store_for(node), node)
            || self.compiler_options.should_preserve_const_enums()
    }

    fn create_export_statement_for_declaration(&mut self, node: ast::Node) -> Option<ast::Node> {
        let namespace = self
            .current_namespace
            .expect("namespace export requires current namespace");
        let namespace_name = self.get_namespace_container_name(namespace);
        let export_name = self.get_external_module_or_namespace_export_name(
            Some(&namespace_name),
            node,
            false,
            true,
        );
        let local_name = self.get_local_name_ex(node, printer::AssignedNameOptions::default());
        let expression = self
            .printer_factory()
            .new_assignment_expression(export_name, local_name);
        let mut export_assignment_source_map_range = self.store_for(node).loc(node);
        if let Some(name) = self.store_for(node).name(node) {
            export_assignment_source_map_range =
                export_assignment_source_map_range.with_pos(self.store_for(name).loc(name).pos());
        }
        self.emit_context
            .set_source_map_range(&expression, export_assignment_source_map_range);

        let statement = self.factory().new_expression_statement(expression);
        self.emit_context
            .set_source_map_range(&statement, self.store_for(node).loc(node).with_pos(-1));
        Some(statement)
    }

    fn create_export_assignment(
        &mut self,
        name: ast::Node,
        expression: ast::Node,
        export_assignment_source_map_range: core::TextRange,
        original: ast::Node,
    ) -> ast::Node {
        let namespace = self
            .current_namespace
            .expect("namespace export requires current namespace");
        let namespace_name = self.get_namespace_container_name(namespace);
        let export_name = self.get_namespace_qualified_property(namespace_name, name);
        let export_assignment = self
            .printer_factory()
            .new_assignment_expression(export_name, expression);
        self.emit_context
            .set_original(&export_assignment, &original);
        self.emit_context
            .set_source_map_range(&export_assignment, export_assignment_source_map_range);
        export_assignment
    }

    fn create_export_statement(
        &mut self,
        name: ast::Node,
        expression: ast::Node,
        export_assignment_source_map_range: core::TextRange,
        export_statement_source_map_range: core::TextRange,
        original: ast::Node,
    ) -> ast::Node {
        let export_assignment = self.create_export_assignment(
            name,
            expression,
            export_assignment_source_map_range,
            original,
        );
        let export_statement = self.factory().new_expression_statement(export_assignment);
        self.emit_context.set_original(&export_statement, &original);
        self.emit_context
            .set_source_map_range(&export_statement, export_statement_source_map_range);
        export_statement
    }

    fn current_scope_is_current_source_file(&self) -> bool {
        match (&self.current_scope, &self.current_source_file) {
            (Some(scope), Some(source_file)) => *scope == *source_file,
            _ => false,
        }
    }

    fn find_super_statement_index(&self, statements: &[ast::Node]) -> Option<usize> {
        statements
            .iter()
            .position(|&statement| self.get_super_call_from_statement(statement))
    }

    fn get_super_call_from_statement(&self, statement: ast::Node) -> bool {
        if !ast::is_expression_statement(self.store_for(statement), statement) {
            return false;
        }
        self.store_for(statement)
            .expression(statement)
            .is_some_and(|expression| {
                let source = self.store_for(expression);
                let expression = ast::skip_parentheses(source, expression);
                ast::is_super_call(self.store_for(expression), expression)
            })
    }

    fn append_visited_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
        changed: &mut bool,
    ) {
        match visited {
            Some(visited) if self.preserved_source_node_matches(Some(original), Some(visited)) => {
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
        if self.is_factory_node(original) {
            original == visited
        } else {
            self.preserved_source_node_matches(Some(original), Some(visited))
        }
    }

    fn preserve_source_node_list_input(
        &mut self,
        nodes: &ast::SourceNodeListInput,
    ) -> ast::NodeList {
        if nodes.store_id() == self.node_factory().store().store_id() {
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
        if modifiers.store_id() == self.node_factory().store().store_id() {
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
        if nodes.store_id() == self.node_factory().store().store_id() {
            return nodes.as_raw_node_slice();
        }
        self.import_state.preserve_source_raw_node_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            nodes,
        )
    }

    fn preserve_source_raw_string_slice_input(
        &mut self,
        strings: &ast::SourceRawStringSliceInput,
    ) -> ast::RawStringSlice {
        if strings.store_id() == self.node_factory().store().store_id() {
            return strings.as_raw_string_slice();
        }
        self.import_state.preserve_source_raw_string_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            strings,
        )
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for RuntimeSyntaxTransformer<'_, 'source> {
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
        self.copy_preserved_emit_metadata(node, imported);
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

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: Option<ast::NodeList>,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        if source_unchanged {
            let imported = self.preserve_node(node);
            return self.record_preserved_node(node, imported);
        }
        if self.is_factory_node(node) {
            self.factory_mut().update_source_file_in_current_store(
                node,
                statements.expect("source file statements cannot be removed"),
                end_of_file_token,
            )
        } else {
            let source = self.source;
            self.import_state.update_source_file_from_store(
                source,
                &mut self.emit_context.factory.node_factory,
                node,
                statements.expect("source file statements cannot be removed"),
                end_of_file_token,
            )
        }
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let mut visited = self.visit(node)?;
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
            match self.visit_node(Some(node)) {
                Some(visited_node)
                    if visited_node == node
                        || self.preserved_source_node_matches(Some(node), Some(visited_node)) =>
                {
                    visited.push(node)
                }
                Some(visited_node) => {
                    changed = true;
                    let children = {
                        let store = self.store_for(visited_node);
                        if store.kind(visited_node) == ast::Kind::SyntaxList {
                            store
                                .syntax_list_children(visited_node)
                                .expect("SyntaxList should have children")
                                .iter()
                                .flatten()
                                .collect::<Vec<_>>()
                        } else {
                            vec![visited_node]
                        }
                    };
                    visited.extend(children);
                }
                None => changed = true,
            }
        }
        if changed {
            visited = self.import_update_nodes(visited);
        }
        let (visited, changed) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        if changed {
            let visited = self.import_update_nodes(visited);
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                nodes.loc(),
                nodes.range(),
                visited,
                nodes.has_trailing_comma(),
            ))
        } else {
            Some(nodes.as_node_list())
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
        let node = node?;
        let visited = self.visit(node);
        let updated = self
            .emit_context
            .finish_visit_embedded_statement(&node, visited);
        updated.map(|updated| self.preserve_node(updated))
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for RuntimeSyntaxTransformer<'_, 'source> {}

fn get_innermost_module_declaration_from_dotted_module(
    source: &ast::AstStore,
    module_declaration: ast::Node,
) -> ast::Node {
    let mut module_declaration = module_declaration;
    while let Some(body) = source.body(module_declaration) {
        if source.kind(body) != ast::Kind::ModuleDeclaration {
            break;
        }
        module_declaration = body;
    }
    module_declaration
}
