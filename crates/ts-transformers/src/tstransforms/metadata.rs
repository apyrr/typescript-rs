use std::collections::HashMap;

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

const USE_NEW_TYPE_METADATA_FORMAT: bool = false;

#[derive(Clone, Default)]
pub struct MetadataResolverFacts {
    type_reference_serialization:
        HashMap<(ast::Node, ast::Node), printer::TypeReferenceSerializationKind>,
}

impl MetadataResolverFacts {
    fn type_reference_serialization_kind(
        &self,
        type_name: ast::Node,
        serial_scope: ast::Node,
    ) -> printer::TypeReferenceSerializationKind {
        *self
            .type_reference_serialization
            .get(&(type_name, serial_scope))
            .expect("missing decorator metadata resolver fact")
    }
}

pub fn collect_metadata_resolver_facts(
    source_file: &ast::SourceFile,
    resolver: &mut dyn printer::EmitResolver,
    compiler_options: &core::CompilerOptions,
) -> MetadataResolverFacts {
    let mut collector = MetadataFactsCollector {
        source: source_file.store(),
        resolver,
        legacy_decorators: compiler_options.experimental_decorators.is_true(),
        current_lexical_scope: None,
        parent: None,
        facts: MetadataResolverFacts::default(),
    };
    collector.visit(source_file.root());
    collector.facts
}

struct MetadataFactsCollector<'a, 'r> {
    source: &'a ast::AstStore,
    resolver: &'r mut dyn printer::EmitResolver,
    legacy_decorators: bool,
    current_lexical_scope: Option<ast::Node>,
    parent: Option<ast::Node>,
    facts: MetadataResolverFacts,
}

impl MetadataFactsCollector<'_, '_> {
    fn visit(&mut self, node: ast::Node) {
        if !self
            .source
            .subtree_facts(node)
            .contains(ast::SubtreeFacts::CONTAINS_DECORATORS)
        {
            return;
        }

        match self.source.kind(node) {
            ast::Kind::SourceFile => {
                let old_parent = self.parent.take();
                let old_scope = self.current_lexical_scope.replace(node);
                self.visit_children(node);
                self.parent = old_parent;
                self.current_lexical_scope = old_scope;
            }
            ast::Kind::ModuleBlock | ast::Kind::Block | ast::Kind::CaseBlock => {
                let old_scope = self.current_lexical_scope.replace(node);
                self.visit_children(node);
                self.current_lexical_scope = old_scope;
            }
            ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => {
                let old_parent = self.parent.replace(node);
                if ast::class_or_constructor_parameter_is_decorated(
                    self.source,
                    self.legacy_decorators,
                    node,
                ) {
                    self.collect_type_metadata(node, node);
                }
                self.visit_children(node);
                self.parent = old_parent;
            }
            ast::Kind::PropertyDeclaration => {
                if let Some(parent) = self.parent
                    && ast::has_decorators(self.source, node)
                    && ast::class_element_or_class_element_parameter_is_decorated(
                        self.source,
                        self.legacy_decorators,
                        node,
                        parent,
                    )
                {
                    self.collect_type_metadata(node, parent);
                }
                self.visit_children(node);
            }
            ast::Kind::MethodDeclaration | ast::Kind::SetAccessor => {
                if let Some(parent) = self.parent
                    && (ast::has_decorators(self.source, node)
                        || !self.get_decorators_of_parameters(node).is_empty())
                    && ast::class_element_or_class_element_parameter_is_decorated(
                        self.source,
                        self.legacy_decorators,
                        node,
                        parent,
                    )
                {
                    self.collect_type_metadata(node, parent);
                }
                self.visit_children(node);
            }
            ast::Kind::GetAccessor => {
                if let Some(parent) = self.parent
                    && ast::has_decorators(self.source, node)
                    && ast::class_element_or_class_element_parameter_is_decorated(
                        self.source,
                        self.legacy_decorators,
                        node,
                        parent,
                    )
                {
                    self.collect_type_metadata(node, parent);
                }
                self.visit_children(node);
            }
            _ => self.visit_children(node),
        }
    }

    fn visit_children(&mut self, node: ast::Node) {
        let mut children = Vec::new();
        let _ = self.source.for_each_present_child(node, |child| {
            children.push(child);
            std::ops::ControlFlow::Continue(())
        });
        for child in children {
            self.visit(child);
        }
    }

    fn get_decorators_of_parameters(&self, node: ast::Node) -> Vec<ast::Node> {
        self.source
            .parameters(node)
            .map(|parameters| {
                parameters
                    .iter()
                    .filter(|parameter| ast::has_decorators(self.source, *parameter))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn collect_type_metadata(&mut self, node: ast::Node, container: ast::Node) {
        if !self.legacy_decorators || USE_NEW_TYPE_METADATA_FORMAT {
            return;
        }
        if matches!(
            self.source.kind(node),
            ast::Kind::MethodDeclaration
                | ast::Kind::GetAccessor
                | ast::Kind::SetAccessor
                | ast::Kind::PropertyDeclaration
        ) {
            self.collect_type_of_node(node, container);
        }
        if self.should_add_param_types_metadata(node) {
            self.collect_parameter_types_of_node(node, container);
        }
        if self.source.kind(node) == ast::Kind::MethodDeclaration {
            self.collect_return_type_of_node(node, container);
        }
    }

    fn should_add_param_types_metadata(&self, node: ast::Node) -> bool {
        match self.source.kind(node) {
            ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => {
                ast::get_first_constructor_with_body(self.source, node).is_some()
            }
            ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor => true,
            _ => false,
        }
    }

    fn collect_type_of_node(&mut self, node: ast::Node, container: ast::Node) {
        match self.source.kind(node) {
            ast::Kind::PropertyDeclaration | ast::Kind::Parameter => {
                self.collect_type_node(self.source.type_node(node), container);
            }
            ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
                self.collect_type_node(self.get_accessor_type_node(node, container), container);
            }
            _ => {}
        }
    }

    fn collect_parameter_types_of_node(&mut self, node: ast::Node, container: ast::Node) {
        let value_declaration = if ast::is_class_like(self.source, node) {
            ast::get_first_constructor_with_body(self.source, node)
        } else if ast::is_function_like(self.source, Some(node)) && self.source.body(node).is_some()
        {
            Some(node)
        } else {
            None
        };
        let Some(value_declaration) = value_declaration else {
            return;
        };
        for parameter in self.parameters_of_decorated_declaration(value_declaration, container) {
            let type_node = if self.source.dot_dot_dot_token(parameter).is_some() {
                ast::get_rest_parameter_element_type(self.source, self.source.type_node(parameter))
            } else {
                self.source.type_node(parameter)
            };
            self.collect_type_node(type_node, container);
        }
    }

    fn collect_return_type_of_node(&mut self, node: ast::Node, container: ast::Node) {
        if ast::is_function_like(self.source, Some(node)) {
            self.collect_type_node(self.source.type_node(node), container);
        }
    }

    fn collect_type_node(&mut self, node: Option<ast::Node>, serial_scope: ast::Node) {
        let Some(node) = node else { return };
        let node = ast::skip_type_parentheses(self.source, node);
        if self.source.kind(node) == ast::Kind::TypeReference
            && let Some(type_name) = self.source.type_name(node)
        {
            let kind = self
                .resolver
                .get_type_reference_serialization_kind(type_name, serial_scope);
            self.facts
                .type_reference_serialization
                .insert((type_name, serial_scope), kind);
        }
        let mut children = Vec::new();
        let _ = self.source.for_each_present_child(node, |child| {
            children.push(child);
            std::ops::ControlFlow::Continue(())
        });
        for child in children {
            self.collect_type_node(Some(child), serial_scope);
        }
    }

    fn get_accessor_type_node(&self, node: ast::Node, container: ast::Node) -> Option<ast::Node> {
        let members = self.source.source_members(container)?;
        let member_nodes = members.iter().collect::<Vec<_>>();
        let accessors = ast::get_all_accessor_declarations(self.source, &member_nodes, node);
        if let Some(set_accessor) = accessors.set_accessor {
            return self
                .get_set_accessor_value_parameter(set_accessor)
                .and_then(|parameter| self.source.type_node(parameter));
        }
        accessors
            .get_accessor
            .and_then(|get_accessor| self.source.type_node(get_accessor))
    }

    fn get_set_accessor_value_parameter(&self, node: ast::Node) -> Option<ast::Node> {
        let parameters = self.source.source_parameters(node)?;
        let parameters = parameters.iter().collect::<Vec<_>>();
        if parameters.len() >= 2 && ast::is_this_parameter(self.source, parameters[0]) {
            return Some(parameters[1]);
        }
        parameters.into_iter().next()
    }

    fn parameters_of_decorated_declaration(
        &self,
        node: ast::Node,
        container: ast::Node,
    ) -> Vec<ast::Node> {
        if self.source.kind(node) == ast::Kind::GetAccessor {
            if let Some(members) = self.source.source_members(container) {
                let member_nodes = members.iter().collect::<Vec<_>>();
                let accessors =
                    ast::get_all_accessor_declarations(self.source, &member_nodes, node);
                if let Some(set_accessor) = accessors.set_accessor {
                    return self
                        .source
                        .source_parameters(set_accessor)
                        .map(|parameters| parameters.iter().collect())
                        .unwrap_or_default();
                }
            }
        }
        self.source
            .source_parameters(node)
            .map(|parameters| parameters.iter().collect())
            .unwrap_or_default()
    }
}

pub fn visit_source_file_output(
    source_file: &ast::SourceFile,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    facts: &MetadataResolverFacts,
) -> ast::Node {
    visit_source_file_root(
        source_file,
        source_file.root(),
        emit_context,
        compiler_options,
        facts,
    )
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
    facts: &MetadataResolverFacts,
) -> ast::Node {
    let mut runtime = MetadataRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        legacy_decorators: compiler_options.experimental_decorators.is_true(),
        strict_null_checks: compiler_options
            .get_strict_option_value(compiler_options.strict_null_checks),
        facts: facts.clone(),
        parent: None,
        current_lexical_scope: None,
        serializing_conditional_type_branch: false,
    };
    let root = runtime.visit_node(Some(root)).unwrap_or(root);
    runtime.emit_context.add_requested_emit_helpers(&root);
    root
}

struct MetadataRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    legacy_decorators: bool,
    strict_null_checks: bool,
    facts: MetadataResolverFacts,
    parent: Option<ast::Node>,
    current_lexical_scope: Option<ast::Node>,
    serializing_conditional_type_branch: bool,
}

impl MetadataRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn visit_each_child(&mut self, node: ast::Node) -> ast::Node {
        self.generated_visit_each_child(&node)
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
            None => {
                *changed = true;
            }
        }
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
        let lifted = if nodes.len() == 1 {
            nodes[0]
        } else {
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
            );
            self.factory_mut().new_block(statements, true)
        };
        assert!(
            self.store_for(lifted).kind(lifted) != ast::Kind::SyntaxList,
            "embedded statement traversal cannot return SyntaxList"
        );
        lifted
    }

    fn visit(&mut self, node: ast::Node) -> Option<ast::Node> {
        let source = self.store_for(node);
        if !source
            .subtree_facts(node)
            .contains(ast::SubtreeFacts::CONTAINS_DECORATORS)
        {
            return Some(node);
        }

        match source.kind(node) {
            ast::Kind::ClassDeclaration => Some(self.visit_class_declaration(node)),
            ast::Kind::ClassExpression => Some(self.visit_class_expression(node)),
            ast::Kind::PropertyDeclaration => Some(self.visit_property_declaration(node)),
            ast::Kind::MethodDeclaration => Some(self.visit_method_declaration(node)),
            ast::Kind::SetAccessor => Some(self.visit_set_accessor(node)),
            ast::Kind::GetAccessor => Some(self.visit_get_accessor(node)),
            ast::Kind::SourceFile => {
                let old_parent = self.parent.take();
                let old_scope = self.current_lexical_scope.replace(node);
                let updated = self.visit_each_child(node);
                self.parent = old_parent;
                self.current_lexical_scope = old_scope;
                Some(updated)
            }
            ast::Kind::ModuleBlock | ast::Kind::Block | ast::Kind::CaseBlock => {
                let old_scope = self.current_lexical_scope.replace(node);
                let updated = self.visit_each_child(node);
                self.current_lexical_scope = old_scope;
                Some(updated)
            }
            _ => Some(self.visit_each_child(node)),
        }
    }

    fn visit_class_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        let old_parent = self.parent.replace(node);
        if !ast::class_or_constructor_parameter_is_decorated(source, self.legacy_decorators, node) {
            let updated = self.visit_each_child(node);
            self.parent = old_parent;
            return updated;
        }
        let modifiers_input =
            (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source);
        let visited_modifiers = self.visit_modifiers_input(modifiers_input.clone());
        let modifiers = self.inject_class_type_metadata(
            visited_modifiers,
            modifiers_input.as_ref(),
            node,
            node,
        );
        let name = self.visit_node(source.name(node));
        let type_parameters = self.visit_nodes_input(
            (source.source_type_parameters(node)).map(ast::SourceNodeListInput::from_source),
        );
        let heritage_clauses = self.visit_nodes_input(
            (source.source_heritage_clauses(node)).map(ast::SourceNodeListInput::from_source),
        );
        let members = self
            .visit_nodes_input(
                (source.source_members(node)).map(ast::SourceNodeListInput::from_source),
            )
            .expect("class members cannot be removed");
        let updated = self.factory_mut().update_class_declaration_from_store(
            source,
            node,
            modifiers,
            name,
            type_parameters,
            heritage_clauses,
            members,
        );
        self.parent = old_parent;
        updated
    }

    fn visit_class_expression(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        let old_parent = self.parent.replace(node);
        if !ast::class_or_constructor_parameter_is_decorated(source, self.legacy_decorators, node) {
            let updated = self.visit_each_child(node);
            self.parent = old_parent;
            return updated;
        }
        let modifiers_input =
            (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source);
        let visited_modifiers = self.visit_modifiers_input(modifiers_input.clone());
        let modifiers = self.inject_class_type_metadata(
            visited_modifiers,
            modifiers_input.as_ref(),
            node,
            node,
        );
        let name = self.visit_node(source.name(node));
        let type_parameters = self.visit_nodes_input(
            (source.source_type_parameters(node)).map(ast::SourceNodeListInput::from_source),
        );
        let heritage_clauses = self.visit_nodes_input(
            (source.source_heritage_clauses(node)).map(ast::SourceNodeListInput::from_source),
        );
        let members = self
            .visit_nodes_input(
                (source.source_members(node)).map(ast::SourceNodeListInput::from_source),
            )
            .expect("class members cannot be removed");
        let updated = self.factory_mut().update_class_expression_from_store(
            source,
            node,
            modifiers,
            name,
            type_parameters,
            heritage_clauses,
            members,
        );
        self.parent = old_parent;
        updated
    }

    fn visit_property_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        if !ast::has_decorators(source, node) {
            return self.visit_each_child(node);
        }
        let modifiers_input =
            (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source);
        let visited_modifiers = self.visit_modifiers_input(modifiers_input.clone());
        let modifiers = self.inject_class_element_type_metadata(
            visited_modifiers,
            modifiers_input.as_ref(),
            node,
            self.parent,
        );
        let name = self.visit_node(source.name(node));
        let postfix_token = self.visit_node(source.postfix_token(node));
        let type_node = self.visit_node(source.type_node(node));
        let initializer = self.visit_node(source.initializer(node));
        let updated = self.factory_mut().update_property_declaration_from_store(
            source,
            node,
            modifiers,
            name,
            postfix_token,
            type_node,
            initializer,
        );
        updated
    }

    fn visit_method_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        if !ast::has_decorators(source, node)
            && self.get_decorators_of_parameters(source, node).is_empty()
        {
            return self.visit_each_child(node);
        }
        let modifiers_input =
            (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source);
        let visited_modifiers = self.visit_modifiers_input(modifiers_input.clone());
        let modifiers = self.inject_class_element_type_metadata(
            visited_modifiers,
            modifiers_input.as_ref(),
            node,
            self.parent,
        );
        let asterisk_token = self.visit_node(source.asterisk_token(node));
        let name = self.visit_node(source.name(node));
        let postfix_token = self.visit_node(source.postfix_token(node));
        let type_parameters = self.visit_nodes_input(
            (source.source_type_parameters(node)).map(ast::SourceNodeListInput::from_source),
        );
        let parameters = self
            .visit_parameters_input(
                (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
            )
            .expect("parameters cannot be removed");
        let type_node = self.visit_node(source.type_node(node));
        let full_signature = self.visit_node(source.full_signature(node));
        let body = self.visit_function_body(source.body(node));
        self.factory_mut().update_method_declaration_from_store(
            source,
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

    fn visit_set_accessor(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        if !ast::has_decorators(source, node)
            && self.get_decorators_of_parameters(source, node).is_empty()
        {
            return self.visit_each_child(node);
        }
        let modifiers_input =
            (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source);
        let visited_modifiers = self.visit_modifiers_input(modifiers_input.clone());
        let modifiers = self.inject_class_element_type_metadata(
            visited_modifiers,
            modifiers_input.as_ref(),
            node,
            self.parent,
        );
        let name = self.visit_node(source.name(node));
        let type_parameters = self.visit_nodes_input(
            (source.source_type_parameters(node)).map(ast::SourceNodeListInput::from_source),
        );
        let parameters = self
            .visit_parameters_input(
                (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
            )
            .expect("parameters cannot be removed");
        let type_node = self.visit_node(source.type_node(node));
        let full_signature = self.visit_node(source.full_signature(node));
        let body = self.visit_function_body(source.body(node));
        self.factory_mut()
            .update_set_accessor_declaration_from_store(
                source,
                node,
                modifiers,
                name,
                type_parameters,
                parameters,
                type_node,
                full_signature,
                body,
            )
    }

    fn visit_get_accessor(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        if !ast::has_decorators(source, node) {
            return self.visit_each_child(node);
        }
        let modifiers_input =
            (source.source_modifiers(node)).map(ast::SourceModifierListInput::from_source);
        let visited_modifiers = self.visit_modifiers_input(modifiers_input.clone());
        let modifiers = self.inject_class_element_type_metadata(
            visited_modifiers,
            modifiers_input.as_ref(),
            node,
            self.parent,
        );
        let name = self.visit_node(source.name(node));
        let type_parameters = self.visit_nodes_input(
            (source.source_type_parameters(node)).map(ast::SourceNodeListInput::from_source),
        );
        let parameters = self
            .visit_parameters_input(
                (source.source_parameters(node)).map(ast::SourceNodeListInput::from_source),
            )
            .expect("parameters cannot be removed");
        let type_node = self.visit_node(source.type_node(node));
        let full_signature = self.visit_node(source.full_signature(node));
        let body = self.visit_function_body(source.body(node));
        self.factory_mut()
            .update_get_accessor_declaration_from_store(
                source,
                node,
                modifiers,
                name,
                type_parameters,
                parameters,
                type_node,
                full_signature,
                body,
            )
    }

    fn inject_class_type_metadata(
        &mut self,
        list: Option<ast::ModifierList>,
        source_list: Option<&ast::SourceModifierListInput>,
        node: ast::Node,
        container: ast::Node,
    ) -> Option<ast::ModifierList> {
        let metadata = self.get_type_metadata(node, container);
        self.inject_metadata_into_modifier_list(list, source_list, metadata, true)
    }

    fn inject_class_element_type_metadata(
        &mut self,
        list: Option<ast::ModifierList>,
        source_list: Option<&ast::SourceModifierListInput>,
        node: ast::Node,
        container: Option<ast::Node>,
    ) -> Option<ast::ModifierList> {
        let Some(container) = container else {
            return list;
        };
        if !ast::is_class_like(self.source, container)
            || !ast::class_element_or_class_element_parameter_is_decorated(
                self.source,
                self.legacy_decorators,
                node,
                container,
            )
        {
            return list;
        }
        let metadata = self.get_type_metadata(node, container);
        self.inject_metadata_into_modifier_list(list, source_list, metadata, false)
    }

    fn inject_metadata_into_modifier_list(
        &mut self,
        list: Option<ast::ModifierList>,
        source_list: Option<&ast::SourceModifierListInput>,
        metadata: Vec<ast::Node>,
        preserve_export_default_prefix: bool,
    ) -> Option<ast::ModifierList> {
        if metadata.is_empty() {
            return list;
        }
        let factory_store_id = self.factory().store().store_id();
        let (original_nodes, loc, range) = if let Some(list) = list {
            if list.store_id() == factory_store_id {
                let store = self.factory().store();
                (
                    store.parser_access().modifier_list_nodes(list),
                    store.transform_access().modifier_list_loc(list),
                    store.transform_access().modifier_list_range(list),
                )
            } else {
                let source_list = source_list
                    .expect("source modifier input is required for a foreign modifier list");
                (
                    source_list
                        .iter()
                        .map(|node| self.preserve_node(node))
                        .collect::<Vec<_>>(),
                    source_list.loc(),
                    source_list.range(),
                )
            }
        } else {
            (
                Vec::new(),
                core::undefined_text_range(),
                core::undefined_text_range(),
            )
        };
        if original_nodes.is_empty() {
            return Some(self.factory_mut().new_modifier_list(
                loc,
                range,
                metadata,
                ast::ModifierFlags::NONE,
            ));
        }

        let store = self.factory().store();
        let mut result = Vec::new();
        let mut rest_start = 0;
        if preserve_export_default_prefix
            && ast::is_modifier(store, original_nodes[0])
            && matches!(
                store.kind(original_nodes[0]),
                ast::Kind::DefaultKeyword | ast::Kind::ExportKeyword
            )
        {
            result.push(original_nodes[0]);
            rest_start = 1;
            if original_nodes.len() > 1
                && ast::is_modifier(store, original_nodes[1])
                && matches!(
                    store.kind(original_nodes[1]),
                    ast::Kind::DefaultKeyword | ast::Kind::ExportKeyword
                )
            {
                result.push(original_nodes[1]);
                rest_start = 2;
            }
        }
        result.extend(
            original_nodes
                .iter()
                .copied()
                .filter(|node| store.kind(*node) == ast::Kind::Decorator),
        );
        result.extend(metadata);
        result.extend(
            original_nodes
                .iter()
                .copied()
                .skip(rest_start)
                .filter(|node| ast::is_modifier(store, *node)),
        );
        Some(
            self.factory_mut()
                .new_modifier_list(loc, range, result, ast::ModifierFlags::NONE),
        )
    }

    fn get_type_metadata(&mut self, node: ast::Node, container: ast::Node) -> Vec<ast::Node> {
        if !self.legacy_decorators || USE_NEW_TYPE_METADATA_FORMAT {
            return Vec::new();
        }
        let mut decorators = Vec::new();
        if matches!(
            self.source.kind(node),
            ast::Kind::MethodDeclaration
                | ast::Kind::GetAccessor
                | ast::Kind::SetAccessor
                | ast::Kind::PropertyDeclaration
        ) {
            let value = self.serialize_type_of_node(node, container);
            decorators.push(self.new_metadata_decorator("design:type", value));
        }
        if self.should_add_param_types_metadata(node, self.source) {
            let value = self.serialize_parameter_types_of_node(node, container);
            decorators.push(self.new_metadata_decorator("design:paramtypes", value));
        }
        if self.source.kind(node) == ast::Kind::MethodDeclaration {
            let value = self.serialize_return_type_of_node(node, container);
            decorators.push(self.new_metadata_decorator("design:returntype", value));
        }
        decorators
    }

    fn should_add_param_types_metadata(&self, node: ast::Node, source: &ast::AstStore) -> bool {
        match source.kind(node) {
            ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => {
                ast::get_first_constructor_with_body(source, node).is_some()
            }
            ast::Kind::MethodDeclaration | ast::Kind::GetAccessor | ast::Kind::SetAccessor => true,
            _ => false,
        }
    }

    fn new_metadata_decorator(&mut self, key: &str, value: ast::Node) -> ast::Node {
        let call = self.emit_context.factory.new_metadata_helper(key, value);
        self.factory_mut().new_decorator(call)
    }

    fn serialize_type_of_node(&mut self, node: ast::Node, container: ast::Node) -> ast::Node {
        let source = self.source;
        match source.kind(node) {
            ast::Kind::PropertyDeclaration | ast::Kind::Parameter => {
                self.serialize_type_node(source.type_node(node), container)
            }
            ast::Kind::GetAccessor | ast::Kind::SetAccessor => self.serialize_type_node(
                self.get_accessor_type_node(source, node, container),
                container,
            ),
            ast::Kind::ClassDeclaration
            | ast::Kind::ClassExpression
            | ast::Kind::MethodDeclaration => self.factory_mut().new_identifier("Function"),
            _ => self.new_void_zero_expression(),
        }
    }

    fn serialize_parameter_types_of_node(
        &mut self,
        node: ast::Node,
        container: ast::Node,
    ) -> ast::Node {
        let source = self.source;
        let value_declaration = if ast::is_class_like(source, node) {
            ast::get_first_constructor_with_body(source, node)
        } else if ast::is_function_like(source, Some(node)) && source.body(node).is_some() {
            Some(node)
        } else {
            None
        };
        let expressions = value_declaration
            .map(|value_declaration| {
                self.parameters_of_decorated_declaration(source, value_declaration, container)
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, parameter)| {
                        if index == 0 && ast::is_this_parameter(source, parameter) {
                            return None;
                        }
                        Some(if source.dot_dot_dot_token(parameter).is_some() {
                            self.serialize_type_node(
                                ast::get_rest_parameter_element_type(
                                    source,
                                    source.type_node(parameter),
                                ),
                                container,
                            )
                        } else {
                            self.serialize_type_of_node(parameter, container)
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let elements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            expressions,
        );
        self.factory_mut()
            .new_array_literal_expression(elements, false)
    }

    fn serialize_return_type_of_node(
        &mut self,
        node: ast::Node,
        container: ast::Node,
    ) -> ast::Node {
        let source = self.source;
        if ast::is_function_like(source, Some(node)) && source.type_node(node).is_some() {
            self.serialize_type_node(source.type_node(node), container)
        } else if ast::is_async_function(source, node) {
            self.factory_mut().new_identifier("Promise")
        } else {
            self.new_void_zero_expression()
        }
    }

    fn serialize_type_node(&mut self, node: Option<ast::Node>, container: ast::Node) -> ast::Node {
        let Some(node) = node else {
            return self.factory_mut().new_identifier("Object");
        };
        let source = self.source;
        let node = ast::skip_type_parentheses(source, node);
        match source.kind(node) {
            ast::Kind::VoidKeyword | ast::Kind::UndefinedKeyword | ast::Kind::NeverKeyword => {
                self.new_void_zero_expression()
            }
            ast::Kind::FunctionType | ast::Kind::ConstructorType => {
                self.factory_mut().new_identifier("Function")
            }
            ast::Kind::ArrayType | ast::Kind::TupleType => {
                self.factory_mut().new_identifier("Array")
            }
            ast::Kind::TypePredicate => {
                if source.asserts_modifier(node).is_some() {
                    self.new_void_zero_expression()
                } else {
                    self.factory_mut().new_identifier("Boolean")
                }
            }
            ast::Kind::BooleanKeyword => self.factory_mut().new_identifier("Boolean"),
            ast::Kind::TemplateLiteralType | ast::Kind::StringKeyword => {
                self.factory_mut().new_identifier("String")
            }
            ast::Kind::ObjectKeyword => self.factory_mut().new_identifier("Object"),
            ast::Kind::LiteralType => self.serialize_literal_type_node(node),
            ast::Kind::NumberKeyword => self.factory_mut().new_identifier("Number"),
            ast::Kind::BigIntKeyword => self.factory_mut().new_identifier("BigInt"),
            ast::Kind::SymbolKeyword => self.factory_mut().new_identifier("Symbol"),
            ast::Kind::TypeReference => self.serialize_type_reference_node(node, container),
            ast::Kind::IntersectionType => self.serialize_union_or_intersection_constituents(
                source.source_types(node),
                true,
                container,
            ),
            ast::Kind::UnionType => self.serialize_union_or_intersection_constituents(
                source.source_types(node),
                false,
                container,
            ),
            ast::Kind::ConditionalType => {
                let true_type = source.true_type(node);
                let false_type = source.false_type(node);
                let types = [true_type, false_type]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                let old_serializing_conditional_type_branch =
                    self.serializing_conditional_type_branch;
                self.serializing_conditional_type_branch = true;
                let result = self.serialize_type_list(types, false, container);
                self.serializing_conditional_type_branch = old_serializing_conditional_type_branch;
                result
            }
            ast::Kind::TypeOperator
                if source.operator(node) == Some(ast::Kind::ReadonlyKeyword) =>
            {
                self.serialize_type_node(source.type_node(node), container)
            }
            ast::Kind::TypeOperator
            | ast::Kind::TypeQuery
            | ast::Kind::IndexedAccessType
            | ast::Kind::MappedType
            | ast::Kind::TypeLiteral
            | ast::Kind::AnyKeyword
            | ast::Kind::UnknownKeyword
            | ast::Kind::ThisType
            | ast::Kind::ImportType => self.factory_mut().new_identifier("Object"),
            _ => self.factory_mut().new_identifier("Object"),
        }
    }

    fn serialize_union_or_intersection_constituents(
        &mut self,
        types: Option<ast::SourceNodeList<'_>>,
        is_intersection: bool,
        container: ast::Node,
    ) -> ast::Node {
        let types = types
            .map(|types| types.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        self.serialize_type_list(types, is_intersection, container)
    }

    fn serialize_type_list(
        &mut self,
        types: Vec<ast::Node>,
        is_intersection: bool,
        container: ast::Node,
    ) -> ast::Node {
        let source = self.source;
        let mut serialized_type = None;
        for type_node in types {
            let type_node = ast::skip_type_parentheses(source, type_node);
            if source.kind(type_node) == ast::Kind::NeverKeyword {
                if is_intersection {
                    return self.new_void_zero_expression();
                }
                continue;
            }
            if source.kind(type_node) == ast::Kind::UnknownKeyword {
                if !is_intersection {
                    return self.factory_mut().new_identifier("Object");
                }
                continue;
            }
            if source.kind(type_node) == ast::Kind::AnyKeyword {
                return self.factory_mut().new_identifier("Object");
            }
            if !self.strict_null_checks
                && (source.kind(type_node) == ast::Kind::UndefinedKeyword
                    || (source.kind(type_node) == ast::Kind::LiteralType
                        && source
                            .literal(type_node)
                            .is_some_and(|literal| source.kind(literal) == ast::Kind::NullKeyword)))
            {
                continue;
            }
            let constituent = self.serialize_type_node(Some(type_node), container);
            if self.is_identifier_text(self.factory().store(), constituent, "Object") {
                return constituent;
            }
            if let Some(previous) = serialized_type {
                if !self.equate_serialized_type_nodes(self.factory().store(), previous, constituent)
                {
                    return self.factory_mut().new_identifier("Object");
                }
            } else {
                serialized_type = Some(constituent);
            }
        }
        serialized_type.unwrap_or_else(|| self.new_void_zero_expression())
    }

    fn serialize_literal_type_node(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        let Some(literal) = source.literal(node) else {
            return self.factory_mut().new_identifier("Object");
        };
        match source.kind(literal) {
            ast::Kind::StringLiteral | ast::Kind::NoSubstitutionTemplateLiteral => {
                self.factory_mut().new_identifier("String")
            }
            ast::Kind::NumericLiteral => self.factory_mut().new_identifier("Number"),
            ast::Kind::BigIntLiteral => self.factory_mut().new_identifier("BigInt"),
            ast::Kind::TrueKeyword | ast::Kind::FalseKeyword => {
                self.factory_mut().new_identifier("Boolean")
            }
            ast::Kind::NullKeyword => self.new_void_zero_expression(),
            ast::Kind::PrefixUnaryExpression => source
                .operand(literal)
                .map(|operand| match source.kind(operand) {
                    ast::Kind::NumericLiteral => self.factory_mut().new_identifier("Number"),
                    ast::Kind::BigIntLiteral => self.factory_mut().new_identifier("BigInt"),
                    _ => self.factory_mut().new_identifier("Object"),
                })
                .unwrap_or_else(|| self.factory_mut().new_identifier("Object")),
            _ => self.factory_mut().new_identifier("Object"),
        }
    }

    fn serialize_type_reference_node(
        &mut self,
        node: ast::Node,
        container: ast::Node,
    ) -> ast::Node {
        let source = self.source;
        let type_name = source
            .type_name(node)
            .expect("type reference should have a type name");
        match self
            .facts
            .type_reference_serialization_kind(type_name, container)
        {
            printer::TypeReferenceSerializationKind::Unknown => {
                // From conditional type type reference that cannot be resolved is Similar to any or unknown
                if self.serializing_conditional_type_branch {
                    return self.factory_mut().new_identifier("Object");
                }

                let serialized = self.serialize_entity_name_as_expression_fallback(type_name);
                let temp = self.emit_context.factory.new_temp_variable();
                self.emit_context.add_variable_declaration(temp);
                let assignment = self.new_assignment_expression(temp, serialized);
                let condition = self.new_type_check(assignment, "function");
                let question_token = self.factory_mut().new_token(ast::Kind::QuestionToken);
                let colon_token = self.factory_mut().new_token(ast::Kind::ColonToken);
                let object = self.factory_mut().new_identifier("Object");
                self.factory_mut().new_conditional_expression(
                    condition,
                    question_token,
                    temp,
                    colon_token,
                    object,
                )
            }
            printer::TypeReferenceSerializationKind::TypeWithConstructSignatureAndValue => {
                self.serialize_entity_name_as_expression(type_name)
            }
            printer::TypeReferenceSerializationKind::VoidNullableOrNeverType => {
                self.new_void_zero_expression()
            }
            printer::TypeReferenceSerializationKind::BigIntLikeType => {
                self.factory_mut().new_identifier("BigInt")
            }
            printer::TypeReferenceSerializationKind::BooleanType => {
                self.factory_mut().new_identifier("Boolean")
            }
            printer::TypeReferenceSerializationKind::NumberLikeType => {
                self.factory_mut().new_identifier("Number")
            }
            printer::TypeReferenceSerializationKind::StringLikeType => {
                self.factory_mut().new_identifier("String")
            }
            printer::TypeReferenceSerializationKind::ArrayLikeType => {
                self.factory_mut().new_identifier("Array")
            }
            printer::TypeReferenceSerializationKind::ESSymbolType => {
                self.factory_mut().new_identifier("Symbol")
            }
            printer::TypeReferenceSerializationKind::TypeWithCallSignature => {
                self.factory_mut().new_identifier("Function")
            }
            printer::TypeReferenceSerializationKind::Promise => {
                self.factory_mut().new_identifier("Promise")
            }
            printer::TypeReferenceSerializationKind::ObjectType => {
                self.factory_mut().new_identifier("Object")
            }
        }
    }

    fn serialize_entity_name_as_expression(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        match source.kind(node) {
            ast::Kind::Identifier => {
                // Create a clone of the name with a new parent, and treat it as if it were
                // a source tree node for the purposes of the checker.
                let name = self
                    .factory_mut()
                    .deep_clone_node_from_store_preserve_location(source, node);
                self.emit_context.unset_original(&name); // make this identifier emulate a parse node, making it behave correctly when inspected by the module transforms
                let parent = self
                    .current_lexical_scope
                    .and_then(|scope| self.emit_context.parse_node(&scope));
                self.factory_mut().link_emit_synthetic_parent(name, parent); // ensure the parent is set to a parse tree node.
                name
            }
            ast::Kind::QualifiedName => {
                let left = source
                    .left(node)
                    .expect("qualified name should have left side");
                let right = source
                    .right(node)
                    .expect("qualified name should have right side");
                let left = self.serialize_entity_name_as_expression(left);
                let right = self.factory_mut().deep_clone_node_from_store(source, right);
                self.factory_mut().new_property_access_expression(
                    left,
                    None,
                    right,
                    ast::NodeFlags::NONE,
                )
            }
            _ => self.factory_mut().new_identifier("Object"),
        }
    }

    fn serialize_entity_name_as_expression_fallback(&mut self, node: ast::Node) -> ast::Node {
        let source = self.source;
        if source.kind(node) == ast::Kind::Identifier {
            // A -> typeof A !== "undefined" && A
            let copied = self.serialize_entity_name_as_expression(node);
            return self.create_checked_value(copied, copied);
        }
        let left_name = source
            .left(node)
            .expect("qualified name should have left side");
        if source.kind(left_name) == ast::Kind::Identifier {
            // A.B -> typeof A !== "undefined" && A.B
            let left = self.serialize_entity_name_as_expression(left_name);
            let right = self.serialize_entity_name_as_expression(node);
            return self.create_checked_value(left, right);
        }
        // A.B.C -> typeof A !== "undefined" && (_a = A.B) !== void 0 && _a.C
        let left = self.serialize_entity_name_as_expression_fallback(left_name);
        let left_store = self.factory().store();
        let left_condition = left_store
            .left(left)
            .expect("fallback expression should be a binary expression");
        let left_value = left_store
            .right(left)
            .expect("fallback expression should be a binary expression");
        let temp = self.emit_context.factory.new_temp_variable();
        self.emit_context.add_variable_declaration(temp);
        let assignment = self.new_assignment_expression(temp, left_value);
        let defined = self.new_defined_check(assignment);
        let check = self.new_logical_and_expression(left_condition, defined);
        let right_name = source
            .right(node)
            .expect("qualified name should have right side");
        let right = self
            .factory_mut()
            .deep_clone_node_from_store(source, right_name);
        let access = self.factory_mut().new_property_access_expression(
            temp,
            None,
            right,
            ast::NodeFlags::NONE,
        );
        self.new_logical_and_expression(check, access)
    }

    fn new_void_zero_expression(&mut self) -> ast::Node {
        let zero = self
            .factory_mut()
            .new_numeric_literal("0", ast::TokenFlags::NONE);
        self.factory_mut().new_void_expression(zero)
    }

    fn create_checked_value(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let type_of = self.factory_mut().new_type_of_expression(left);
        let undefined = self
            .factory_mut()
            .new_string_literal("undefined", ast::TokenFlags::NONE);
        let check = self.new_strict_inequality_expression(type_of, undefined);
        self.new_logical_and_expression(check, right)
    }

    fn new_defined_check(&mut self, value: ast::Node) -> ast::Node {
        let void_zero = self.new_void_zero_expression();
        self.new_strict_inequality_expression(value, void_zero)
    }

    fn new_type_check(&mut self, value: ast::Node, tag: &str) -> ast::Node {
        let type_of = self.factory_mut().new_type_of_expression(value);
        let tag = self
            .factory_mut()
            .new_string_literal(tag, ast::TokenFlags::NONE);
        self.new_strict_equality_expression(type_of, tag)
    }

    fn new_logical_and_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self
            .factory_mut()
            .new_token(ast::Kind::AmpersandAmpersandToken);
        self.factory_mut()
            .new_binary_expression(None, left, None, operator, right)
    }

    fn new_assignment_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self.factory_mut().new_token(ast::Kind::EqualsToken);
        self.factory_mut()
            .new_binary_expression(None, left, None, operator, right)
    }

    fn new_strict_equality_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self
            .factory_mut()
            .new_token(ast::Kind::EqualsEqualsEqualsToken);
        self.factory_mut()
            .new_binary_expression(None, left, None, operator, right)
    }

    fn new_strict_inequality_expression(&mut self, left: ast::Node, right: ast::Node) -> ast::Node {
        let operator = self
            .factory_mut()
            .new_token(ast::Kind::ExclamationEqualsEqualsToken);
        self.factory_mut()
            .new_binary_expression(None, left, None, operator, right)
    }

    fn is_identifier_text(&self, store: &ast::AstStore, node: ast::Node, text: &str) -> bool {
        store.kind(node) == ast::Kind::Identifier && store.text(node) == text
    }

    fn equate_serialized_type_nodes(
        &self,
        store: &ast::AstStore,
        left: ast::Node,
        right: ast::Node,
    ) -> bool {
        match (store.kind(left), store.kind(right)) {
            (ast::Kind::Identifier, ast::Kind::Identifier) => store.text(left) == store.text(right),
            (ast::Kind::PropertyAccessExpression, ast::Kind::PropertyAccessExpression) => {
                source_pair(store.expression(left), store.expression(right)).is_some_and(
                    |(left, right)| self.equate_serialized_type_nodes(store, left, right),
                ) && source_pair(store.name(left), store.name(right)).is_some_and(
                    |(left, right)| self.equate_serialized_type_nodes(store, left, right),
                )
            }
            (ast::Kind::VoidExpression, ast::Kind::VoidExpression) => true,
            _ => false,
        }
    }

    fn get_decorators_of_parameters(
        &self,
        source: &ast::AstStore,
        node: ast::Node,
    ) -> Vec<ast::Node> {
        source
            .parameters(node)
            .map(|parameters| {
                parameters
                    .iter()
                    .filter(|parameter| ast::has_decorators(source, *parameter))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_accessor_type_node(
        &self,
        source: &ast::AstStore,
        node: ast::Node,
        container: ast::Node,
    ) -> Option<ast::Node> {
        let members = source.source_members(container)?;
        let member_nodes = members.iter().collect::<Vec<_>>();
        let accessors = ast::get_all_accessor_declarations(source, &member_nodes, node);
        if let Some(set_accessor) = accessors.set_accessor {
            return self
                .get_set_accessor_value_parameter(source, set_accessor)
                .and_then(|parameter| source.type_node(parameter));
        }
        accessors
            .get_accessor
            .and_then(|get_accessor| source.type_node(get_accessor))
    }

    fn get_set_accessor_value_parameter(
        &self,
        source: &ast::AstStore,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let parameters = source.source_parameters(node)?;
        let parameters = parameters.iter().collect::<Vec<_>>();
        if parameters.len() >= 2 && ast::is_this_parameter(source, parameters[0]) {
            return Some(parameters[1]);
        }
        parameters.into_iter().next()
    }

    fn parameters_of_decorated_declaration(
        &self,
        source: &ast::AstStore,
        node: ast::Node,
        container: ast::Node,
    ) -> Vec<ast::Node> {
        if source.kind(node) == ast::Kind::GetAccessor
            && let Some(members) = source.source_members(container)
        {
            let member_nodes = members.iter().collect::<Vec<_>>();
            let accessors = ast::get_all_accessor_declarations(source, &member_nodes, node);
            if let Some(set_accessor) = accessors.set_accessor {
                return source
                    .source_parameters(set_accessor)
                    .map(|parameters| parameters.iter().collect())
                    .unwrap_or_default();
            }
        }
        source
            .source_parameters(node)
            .map(|parameters| parameters.iter().collect())
            .unwrap_or_default()
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for MetadataRuntime<'_, 'source> {
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
        self.import_state
            .preserve_node(source, &mut self.emit_context.factory.node_factory, node)
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
            return self.factory_mut().update_source_file_in_current_store(
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
            assert!(
                self.store_for(visited).kind(visited) != ast::Kind::SyntaxList,
                "single-node metadata traversal cannot return SyntaxList"
            );
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
            let result = self.visit(node);
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
            Some(nodes.as_node_list())
        }
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;

        self.emit_context.start_variable_environment();
        for node in source_list.iter() {
            let result = self.visit(node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        let declarations = self.emit_context.end_variable_environment();
        let (visited, environment_changed) = self
            .emit_context
            .merge_environment_for_resolved_nodes(&visited, &declarations);

        if changed || environment_changed {
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                visited,
                source_list.has_trailing_comma(),
            ))
        } else {
            Some(self.import_state.preserve_source_node_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            ))
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
                let visited = self.visit(node);
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for MetadataRuntime<'_, 'source> {}

fn source_pair(
    left: Option<ast::Node>,
    right: Option<ast::Node>,
) -> Option<(ast::Node, ast::Node)> {
    left.zip(right)
}
