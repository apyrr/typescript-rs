use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalAssignmentAction {
    Keep,
    VisitChildren,
    LowerLogicalAssignment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalAssignmentOperator {
    Or,
    And,
    Nullish,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LogicalAssignmentFacts {
    pub subtree_contains_logical_assignments: bool,
    pub operator_is_or_assignment: bool,
    pub operator_is_and_assignment: bool,
    pub operator_is_nullish_assignment: bool,
    pub left_is_access_expression: bool,
    pub access_target_is_simple_copiable: bool,
    pub element_argument_is_simple_copiable: bool,
}

pub fn logical_assignment_action_for_kind(
    kind: ast::Kind,
    facts: LogicalAssignmentFacts,
) -> LogicalAssignmentAction {
    if !facts.subtree_contains_logical_assignments {
        return LogicalAssignmentAction::Keep;
    }

    match kind {
        ast::Kind::BinaryExpression if logical_assignment_operator(facts).is_some() => {
            LogicalAssignmentAction::LowerLogicalAssignment
        }
        _ => LogicalAssignmentAction::VisitChildren,
    }
}

pub fn logical_assignment_operator(
    facts: LogicalAssignmentFacts,
) -> Option<LogicalAssignmentOperator> {
    if facts.operator_is_or_assignment {
        Some(LogicalAssignmentOperator::Or)
    } else if facts.operator_is_and_assignment {
        Some(LogicalAssignmentOperator::And)
    } else if facts.operator_is_nullish_assignment {
        Some(LogicalAssignmentOperator::Nullish)
    } else {
        None
    }
}

pub fn access_target_needs_temp(facts: LogicalAssignmentFacts) -> bool {
    facts.left_is_access_expression && !facts.access_target_is_simple_copiable
}

pub fn element_argument_needs_temp(facts: LogicalAssignmentFacts) -> bool {
    facts.left_is_access_expression && !facts.element_argument_is_simple_copiable
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    let mut runtime = LogicalAssignmentTransformer {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

struct LogicalAssignmentTransformer<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
}

impl LogicalAssignmentTransformer<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        if !source
            .subtree_facts(*node)
            .intersects(ast::SubtreeFacts::CONTAINS_LOGICAL_ASSIGNMENTS)
        {
            return Some(*node);
        }

        match source.kind(*node) {
            ast::Kind::BinaryExpression => Some(self.visit_binary_expression(*node)),
            _ => Some(self.generated_visit_each_child(node)),
        }
    }

    fn visit_binary_expression(&mut self, node: ast::Node) -> ast::Node {
        let (operator_kind, left_node, right_node) = {
            let source = self.store_for(node);
            (
                source
                    .operator_token(node)
                    .map(|token| source.kind(token))
                    .expect("binary expression should have operator token"),
                source.left(node),
                source.right(node),
            )
        };
        let non_assignment_operator = match operator_kind {
            ast::Kind::BarBarEqualsToken => ast::Kind::BarBarToken,
            ast::Kind::AmpersandAmpersandEqualsToken => ast::Kind::AmpersandAmpersandToken,
            ast::Kind::QuestionQuestionEqualsToken => ast::Kind::QuestionQuestionToken,
            _ => return self.generated_visit_each_child(&node),
        };

        let visited_left = self
            .visit_node(left_node)
            .expect("binary expression should have left");
        let mut left = ast::skip_parentheses(self.store_for(visited_left), visited_left);
        let mut assignment_target = left;
        let visited_right = self
            .visit_node(right_node)
            .expect("binary expression should have right");
        let right = ast::skip_parentheses(self.store_for(visited_right), visited_right);

        if ast::is_access_expression(self.store_for(left), left) {
            let expression = self
                .store_for(left)
                .expression(left)
                .expect("access expression should have expression");
            let mut property_access_target = expression;
            let mut property_access_target_assignment = expression;
            if !crate::utilities::is_simple_copiable_expression(
                self.store_for(expression),
                &expression,
            ) {
                property_access_target = self.emit_context.factory.new_temp_variable();
                self.emit_context
                    .add_variable_declaration(property_access_target);
                property_access_target_assignment = self
                    .emit_context
                    .factory
                    .new_assignment_expression(property_access_target, expression);
            }

            if self.store_for(left).kind(left) == ast::Kind::PropertyAccessExpression {
                let name = self
                    .store_for(left)
                    .name(left)
                    .expect("property access expression should have name");
                assignment_target = self.factory_mut().new_property_access_expression(
                    property_access_target,
                    None,
                    name,
                    ast::NodeFlags::NONE,
                );
                left = self.factory_mut().new_property_access_expression(
                    property_access_target_assignment,
                    None,
                    name,
                    ast::NodeFlags::NONE,
                );
            } else {
                let argument = self
                    .store_for(left)
                    .argument_expression(left)
                    .expect("element access expression should have argument expression");
                let mut element_access_argument = argument;
                let mut argument_expr = argument;
                if !crate::utilities::is_simple_copiable_expression(
                    self.store_for(argument),
                    &argument,
                ) {
                    element_access_argument = self.emit_context.factory.new_temp_variable();
                    self.emit_context
                        .add_variable_declaration(element_access_argument);
                    argument_expr = self
                        .emit_context
                        .factory
                        .new_assignment_expression(element_access_argument, argument);
                }

                assignment_target = self.factory_mut().new_element_access_expression(
                    property_access_target,
                    None,
                    element_access_argument,
                    ast::NodeFlags::NONE,
                );
                left = self.factory_mut().new_element_access_expression(
                    property_access_target_assignment,
                    None,
                    argument_expr,
                    ast::NodeFlags::NONE,
                );
            }
        }

        let operator = self.factory_mut().new_token(non_assignment_operator);
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(assignment_target, right);
        let parenthesized = self.factory_mut().new_parenthesized_expression(assignment);
        self.factory_mut()
            .new_binary_expression(None, left, None, operator, parenthesized)
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

impl<'source> ast::AstVisitEachChildRuntime<'source> for LogicalAssignmentTransformer<'_, 'source> {
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

    fn visit_nodes_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;
        for node in source_list.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        if changed {
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

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        Some(self.import_state.preserve_source_modifier_list_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &modifiers,
        ))
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let old_flags = self.emit_context.begin_visit_parameters();
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;
        for node in source_list.iter() {
            let result = self.visit(&node);
            self.append_visited_node(node, result, &mut visited, &mut changed);
        }
        let (visited, changed) = self
            .emit_context
            .finish_visit_parameters(old_flags, visited, changed);
        if changed {
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
            let result = self.visit(&node);
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

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        Some(self.import_state.preserve_source_raw_node_slice_input(
            self.source,
            &mut self.emit_context.factory.node_factory,
            &nodes,
        ))
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source>
    for LogicalAssignmentTransformer<'_, 'source>
{
}
