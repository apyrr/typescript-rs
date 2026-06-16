use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_printer as printer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExponentiationAction {
    Keep,
    VisitChildren,
    LowerExponentiation,
    LowerExponentiationAssignment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExponentiationAssignmentTarget {
    IdentifierOrOther,
    PropertyAccessNeedsObjectTemp,
    ElementAccessNeedsObjectAndArgumentTemps,
}

pub fn exponentiation_action_for_kind(
    kind: ast::Kind,
    subtree_contains_exponentiation_operator: bool,
    operator_is_exponentiation: bool,
    operator_is_exponentiation_assignment: bool,
) -> ExponentiationAction {
    if !subtree_contains_exponentiation_operator {
        return ExponentiationAction::Keep;
    }

    match kind {
        ast::Kind::BinaryExpression if operator_is_exponentiation_assignment => {
            ExponentiationAction::LowerExponentiationAssignment
        }
        ast::Kind::BinaryExpression if operator_is_exponentiation => {
            ExponentiationAction::LowerExponentiation
        }
        _ => ExponentiationAction::VisitChildren,
    }
}

pub fn exponentiation_assignment_target(
    left_is_property_access: bool,
    left_is_element_access: bool,
) -> ExponentiationAssignmentTarget {
    if left_is_element_access {
        ExponentiationAssignmentTarget::ElementAccessNeedsObjectAndArgumentTemps
    } else if left_is_property_access {
        ExponentiationAssignmentTarget::PropertyAccessNeedsObjectTemp
    } else {
        ExponentiationAssignmentTarget::IdentifierOrOther
    }
}

pub fn exponentiation_helper_name() -> (&'static str, &'static str) {
    ("Math", "pow")
}
use ts_ast as ast;

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    let mut runtime = ExponentiationTransformer {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

struct ExponentiationTransformer<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
}

impl ExponentiationTransformer<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn set_loc(&mut self, node: ast::Node, loc: ts_core::TextRange) -> ast::Node {
        self.factory_mut().place_emit_synthetic_node(node, loc);
        node
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        if !source
            .subtree_facts(*node)
            .intersects(ast::SubtreeFacts::CONTAINS_EXPONENTIATION_OPERATOR)
        {
            return Some(*node);
        }
        match source.kind(*node) {
            ast::Kind::BinaryExpression => Some(self.visit_binary_expression(*node)),
            _ => Some(self.generated_visit_each_child(node)),
        }
    }

    fn visit_binary_expression(&mut self, node: ast::Node) -> ast::Node {
        let operator_kind = {
            let source = self.store_for(node);
            source
                .operator_token(node)
                .map(|token| source.kind(token))
                .expect("binary expression should have operator token")
        };
        match operator_kind {
            ast::Kind::AsteriskAsteriskEqualsToken => {
                self.visit_exponentiation_assignment_expression(node)
            }
            ast::Kind::AsteriskAsteriskToken => self.visit_exponentiation_expression(node),
            _ => self.generated_visit_each_child(&node),
        }
    }

    fn visit_exponentiation_assignment_expression(&mut self, node: ast::Node) -> ast::Node {
        let (original_left, original_right, node_loc) = {
            let source = self.store_for(node);
            (source.left(node), source.right(node), source.loc(node))
        };
        let left = self
            .visit_node(original_left)
            .expect("binary expression should have left");
        let right = self
            .visit_node(original_right)
            .expect("binary expression should have right");
        let left_kind = self.store_for(left).kind(left);
        let (target, value) = if left_kind == ast::Kind::ElementAccessExpression {
            // Transforms `a[x] **= b` into `(_a = a)[_x = x] = Math.pow(_a[_x], b)`
            let (expression, argument_expression, expression_loc, argument_loc, left_loc) = {
                let left_store = self.store_for(left);
                let expression = left_store
                    .expression(left)
                    .expect("element access should have expression");
                let argument_expression = left_store
                    .argument_expression(left)
                    .expect("element access should have argument expression");
                (
                    expression,
                    argument_expression,
                    left_store.loc(expression),
                    left_store.loc(argument_expression),
                    left_store.loc(left),
                )
            };
            let expression_temp = self.emit_context.factory.new_temp_variable();
            self.emit_context.add_variable_declaration(expression_temp);
            let argument_expression_temp = self.emit_context.factory.new_temp_variable();
            self.emit_context
                .add_variable_declaration(argument_expression_temp);

            let obj_expr = self
                .emit_context
                .factory
                .new_assignment_expression(expression_temp, expression);
            let obj_expr = self.set_loc(obj_expr, expression_loc);
            let access_expr = self
                .emit_context
                .factory
                .new_assignment_expression(argument_expression_temp, argument_expression);
            let access_expr = self.set_loc(access_expr, argument_loc);

            let target = self.factory_mut().new_element_access_expression(
                obj_expr,
                None::<ast::Node>,
                access_expr,
                ast::NodeFlags::NONE,
            );
            let value = self.factory_mut().new_element_access_expression(
                expression_temp,
                None::<ast::Node>,
                argument_expression_temp,
                ast::NodeFlags::NONE,
            );
            let value = self.set_loc(value, left_loc);
            (target, value)
        } else if left_kind == ast::Kind::PropertyAccessExpression {
            // Transforms `a.x **= b` into `(_a = a).x = Math.pow(_a.x, b)`
            let (expression, name, expression_loc, left_loc) = {
                let left_store = self.store_for(left);
                let expression = left_store
                    .expression(left)
                    .expect("property access should have expression");
                let name = left_store
                    .name(left)
                    .expect("property access should have name");
                (
                    expression,
                    name,
                    left_store.loc(expression),
                    left_store.loc(left),
                )
            };
            let expression_temp = self.emit_context.factory.new_temp_variable();
            self.emit_context.add_variable_declaration(expression_temp);
            let assignment = self
                .emit_context
                .factory
                .new_assignment_expression(expression_temp, expression);
            let assignment = self.set_loc(assignment, expression_loc);
            let target = self.factory_mut().new_property_access_expression(
                assignment,
                None::<ast::Node>,
                name,
                ast::NodeFlags::NONE,
            );
            let target = self.set_loc(target, left_loc);

            let value = self.factory_mut().new_property_access_expression(
                expression_temp,
                None::<ast::Node>,
                name,
                ast::NodeFlags::NONE,
            );
            let value = self.set_loc(value, left_loc);
            (target, value)
        } else {
            // Transforms `a **= b` into `a = Math.pow(a, b)`
            (left, left)
        };

        let rhs = self
            .emit_context
            .factory
            .new_global_method_call("Math", "pow", &[value, right]);
        let rhs = self.set_loc(rhs, node_loc);
        let result = self
            .emit_context
            .factory
            .new_assignment_expression(target, rhs);
        self.set_loc(result, node_loc)
    }

    fn visit_exponentiation_expression(&mut self, node: ast::Node) -> ast::Node {
        let (original_left, original_right, node_loc) = {
            let source = self.store_for(node);
            (source.left(node), source.right(node), source.loc(node))
        };
        let left = self
            .visit_node(original_left)
            .expect("binary expression should have left");
        let right = self
            .visit_node(original_right)
            .expect("binary expression should have right");
        let result =
            self.emit_context
                .factory
                .new_global_method_call("Math", "pow", &[left, right]);
        self.set_loc(result, node_loc)
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
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for ExponentiationTransformer<'_, 'source> {
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
        self.visit_node(node)
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for ExponentiationTransformer<'_, 'source> {}
