use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

use super::utilities::{self, SuperAccessState};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ForAwaitHierarchyFacts {
    pub has_lexical_this: bool,
    pub iteration_container: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForAwaitAction {
    KeepFallback,
    VisitChildren,
    TransformSourceFile,
    RewriteAwaitInAsyncGenerator,
    RewriteYieldInAsyncGenerator,
    RewriteReturnInAsyncGenerator,
    VisitLabeledStatement,
    VisitForOfStatement,
    VisitIterationStatement,
    VisitFunctionLike,
    VisitArrowFunction,
    VisitClassOrFunctionBoundary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncGeneratorYieldAction {
    AwaitHelperYield,
    AwaitAsyncDelegatedYield,
    VisitChildren,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuperHelperKind {
    None,
    AsyncSuper,
    AdvancedAsyncSuper,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ForAwaitFacts {
    pub subtree_contains_for_await_or_async_generator: bool,
    pub enclosing_function_is_async: bool,
    pub enclosing_function_is_generator: bool,
    pub for_of_has_await_modifier: bool,
    pub labeled_inner_is_for_await: bool,
    pub function_is_async: bool,
    pub function_is_generator: bool,
    pub parameter_list_is_simple: bool,
    pub has_super_element_access: bool,
    pub has_super_property_assignment: bool,
    pub captured_super_property_count: usize,
}

pub fn for_await_action_for_kind(kind: ast::Kind, facts: ForAwaitFacts) -> ForAwaitAction {
    if !facts.subtree_contains_for_await_or_async_generator {
        return ForAwaitAction::KeepFallback;
    }

    match kind {
        ast::Kind::SourceFile => ForAwaitAction::TransformSourceFile,
        ast::Kind::AwaitExpression if in_async_generator(facts) => {
            ForAwaitAction::RewriteAwaitInAsyncGenerator
        }
        ast::Kind::YieldExpression if in_async_generator(facts) => {
            ForAwaitAction::RewriteYieldInAsyncGenerator
        }
        ast::Kind::ReturnStatement if in_async_generator(facts) => {
            ForAwaitAction::RewriteReturnInAsyncGenerator
        }
        ast::Kind::LabeledStatement if facts.enclosing_function_is_async => {
            ForAwaitAction::VisitLabeledStatement
        }
        ast::Kind::DoStatement
        | ast::Kind::WhileStatement
        | ast::Kind::ForInStatement
        | ast::Kind::ForStatement => ForAwaitAction::VisitIterationStatement,
        ast::Kind::ForOfStatement => ForAwaitAction::VisitForOfStatement,
        ast::Kind::Constructor
        | ast::Kind::MethodDeclaration
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression => ForAwaitAction::VisitFunctionLike,
        ast::Kind::ArrowFunction => ForAwaitAction::VisitArrowFunction,
        ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => {
            ForAwaitAction::VisitClassOrFunctionBoundary
        }
        _ => ForAwaitAction::VisitChildren,
    }
}

pub fn in_async_generator(facts: ForAwaitFacts) -> bool {
    facts.enclosing_function_is_async && facts.enclosing_function_is_generator
}

pub fn yield_action(
    in_async_generator: bool,
    has_asterisk_token: bool,
) -> AsyncGeneratorYieldAction {
    if !in_async_generator {
        AsyncGeneratorYieldAction::VisitChildren
    } else if has_asterisk_token {
        AsyncGeneratorYieldAction::AwaitAsyncDelegatedYield
    } else {
        AsyncGeneratorYieldAction::AwaitHelperYield
    }
}

pub fn for_of_statement_needs_downlevel(for_of_has_await_modifier: bool) -> bool {
    for_of_has_await_modifier
}

pub fn create_downlevel_await_uses_yield(enclosing_function_is_generator: bool) -> bool {
    enclosing_function_is_generator
}

pub fn remove_async_modifier(function_is_generator: bool) -> bool {
    function_is_generator
}

pub fn remove_asterisk_token(function_is_async: bool) -> bool {
    function_is_async
}

pub fn async_generator_needs_fixed_parameter_list(parameter_list_is_simple: bool) -> bool {
    !parameter_list_is_simple
}

pub fn super_helper_kind(facts: ForAwaitFacts) -> SuperHelperKind {
    if !facts.has_super_element_access {
        SuperHelperKind::None
    } else if facts.has_super_property_assignment {
        SuperHelperKind::AdvancedAsyncSuper
    } else {
        SuperHelperKind::AsyncSuper
    }
}

pub fn emits_super_access_variable(captured_super_property_count: usize) -> bool {
    captured_super_property_count > 0
}

pub fn enter_iteration_container(mut facts: ForAwaitHierarchyFacts) -> ForAwaitHierarchyFacts {
    facts.iteration_container = true;
    facts
}

pub fn enter_class_or_function_boundary(
    mut facts: ForAwaitHierarchyFacts,
) -> ForAwaitHierarchyFacts {
    facts.has_lexical_this = true;
    facts.iteration_container = false;
    facts
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    if source_file.is_declaration_file() {
        return root;
    }

    let mut runtime = ForAwaitRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        enclosing_function_flags: ast::FUNCTION_FLAGS_NORMAL,
        hierarchy_facts: ForAwaitHierarchyFacts::default(),
        super_access_state: None,
        super_binding: None,
        super_index_binding: None,
        substitute_super_accesses: false,
        parent_node: None,
        current_node: None,
    };
    let root = runtime.visit_node(Some(root)).unwrap_or(root);
    runtime.emit_context.add_requested_emit_helpers(&root);
    root
}

struct ForAwaitRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    enclosing_function_flags: ast::FunctionFlags,
    hierarchy_facts: ForAwaitHierarchyFacts,
    super_access_state: Option<SuperAccessState>,
    super_binding: Option<ast::Node>,
    super_index_binding: Option<ast::Node>,
    substitute_super_accesses: bool,
    parent_node: Option<ast::Node>,
    current_node: Option<ast::Node>,
}

impl ForAwaitRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn source_parameters_input(&self, node: ast::Node) -> Option<ast::SourceNodeListInput> {
        self.store_for(node)
            .source_parameters(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn new_generated_name_for_node_ex(
        &mut self,
        node: ast::Node,
        options: printer::AutoGenerateOptions,
    ) -> ast::Node {
        let original = self.emit_context.most_original(&node);
        if original.store_id() == self.source.store_id() {
            return self.emit_context.factory.new_generated_name_for_node_ex(
                self.source,
                &original,
                options,
            );
        }
        if let Some(source_file) = self.emit_context.source_file_handle_for_node(original) {
            return self.emit_context.factory.new_generated_name_for_node_ex(
                source_file.store(),
                &original,
                options,
            );
        }
        if node.store_id() == self.source.store_id() {
            return self.emit_context.factory.new_generated_name_for_node_ex(
                self.source,
                &node,
                options,
            );
        }
        self.emit_context
            .factory
            .new_generated_name_for_factory_node_ex(&node, options)
    }

    fn is_factory_node(&self, node: ast::Node) -> bool {
        node.store_id() == self.factory().store().store_id()
    }

    fn restore_enclosing_label(
        &mut self,
        node: &ast::Node,
        outermost_labeled_statement: Option<&ast::Node>,
    ) -> ast::Node {
        let Some(outermost_labeled_statement) = outermost_labeled_statement else {
            return *node;
        };
        if outermost_labeled_statement.store_id() != self.factory().store().store_id() {
            return self.emit_context.factory.restore_enclosing_label(
                self.source,
                node,
                Some(outermost_labeled_statement),
            );
        }

        let statement = self
            .factory()
            .store()
            .statement(*outermost_labeled_statement)
            .expect("labeled statement should have a statement");
        let inner_label = if ast::is_labeled_statement(self.factory().store(), statement) {
            self.restore_enclosing_label(node, Some(&statement))
        } else {
            *node
        };
        let label = self
            .factory()
            .store()
            .label(*outermost_labeled_statement)
            .expect("labeled statement should have a label");
        self.factory_mut().update_labeled_statement(
            *outermost_labeled_statement,
            label,
            inner_label,
        )
    }

    fn descend_into<R>(&mut self, node: ast::Node, cb: impl FnOnce(&mut Self) -> R) -> R {
        let saved_parent = self.parent_node;
        let saved_current = self.current_node;
        self.parent_node = self.current_node;
        self.current_node = Some(node);
        let result = cb(self);
        self.current_node = saved_current;
        self.parent_node = saved_parent;
        result
    }

    fn with_super_substitution<R>(&mut self, cb: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.substitute_super_accesses;
        self.substitute_super_accesses = true;
        let result = cb(self);
        self.substitute_super_accesses = saved;
        result
    }

    fn with_enclosing_function_flags<R>(
        &mut self,
        flags: ast::FunctionFlags,
        cb: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let saved = self.enclosing_function_flags;
        self.enclosing_function_flags = flags;
        let result = cb(self);
        self.enclosing_function_flags = saved;
        result
    }

    fn with_hierarchy_facts<R>(
        &mut self,
        facts: ForAwaitHierarchyFacts,
        cb: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let saved = self.hierarchy_facts;
        self.hierarchy_facts = facts;
        let result = cb(self);
        self.hierarchy_facts = saved;
        result
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let kind = self.store_for(*node).kind(*node);
        self.descend_into(*node, |this| {
            if this.substitute_super_accesses
                && let Some(substituted) = this.try_substitute_super_access(*node)
            {
                return Some(substituted);
            }
            if !this.contains_for_await_transform(*node) {
                return Some(this.generated_visit_each_child(node));
            }
            match kind {
                ast::Kind::SourceFile => Some(this.generated_visit_each_child(node)),
                ast::Kind::AwaitExpression if this.in_async_generator() => {
                    Some(this.visit_await_expression(*node))
                }
                ast::Kind::YieldExpression if this.in_async_generator() => {
                    Some(this.visit_yield_expression(*node))
                }
                ast::Kind::ReturnStatement if this.in_async_generator() => {
                    Some(this.visit_return_statement(*node))
                }
                ast::Kind::LabeledStatement
                    if this.enclosing_function_flags & ast::FUNCTION_FLAGS_ASYNC != 0 =>
                {
                    Some(this.visit_labeled_statement(*node))
                }
                ast::Kind::MethodDeclaration => {
                    let facts = enter_class_or_function_boundary(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.visit_method_declaration(*node))
                    })
                }
                ast::Kind::FunctionDeclaration => {
                    let facts = enter_class_or_function_boundary(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.visit_function_declaration(*node))
                    })
                }
                ast::Kind::FunctionExpression => {
                    let facts = enter_class_or_function_boundary(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.visit_function_expression(*node))
                    })
                }
                ast::Kind::GetAccessor => {
                    let facts = enter_class_or_function_boundary(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.visit_get_accessor_declaration(*node))
                    })
                }
                ast::Kind::SetAccessor => {
                    let facts = enter_class_or_function_boundary(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.visit_set_accessor_declaration(*node))
                    })
                }
                ast::Kind::Constructor => {
                    let facts = enter_class_or_function_boundary(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.visit_constructor_declaration(*node))
                    })
                }
                ast::Kind::ArrowFunction => Some(this.visit_arrow_function(*node)),
                ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => {
                    let facts = enter_class_or_function_boundary(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.generated_visit_each_child(node))
                    })
                }
                ast::Kind::DoStatement
                | ast::Kind::WhileStatement
                | ast::Kind::ForInStatement
                | ast::Kind::ForStatement => {
                    let facts = enter_iteration_container(this.hierarchy_facts);
                    this.with_hierarchy_facts(facts, |this| {
                        Some(this.generated_visit_each_child(node))
                    })
                }
                ast::Kind::ForOfStatement => Some(this.visit_for_of_statement(*node, None)),
                _ => Some(this.generated_visit_each_child(node)),
            }
        })
    }

    fn contains_for_await_transform(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        store
            .subtree_facts(node)
            .contains(ast::SubtreeFacts::CONTAINS_FOR_AWAIT_OR_ASYNC_GENERATOR)
            || matches!(
                store.kind(node),
                ast::Kind::AwaitExpression
                    | ast::Kind::YieldExpression
                    | ast::Kind::ReturnStatement
                    | ast::Kind::ForOfStatement
            )
            || ast::get_function_flags(store, Some(node)) & ast::FUNCTION_FLAGS_ASYNC_GENERATOR
                == ast::FUNCTION_FLAGS_ASYNC_GENERATOR
            || {
                let mut found = false;
                let _ = store.for_each_present_child(node, |child| {
                    if self.contains_for_await_transform(child) {
                        found = true;
                        std::ops::ControlFlow::Break(())
                    } else {
                        std::ops::ControlFlow::Continue(())
                    }
                });
                found
            }
    }

    fn in_async_generator(&self) -> bool {
        self.enclosing_function_flags & ast::FUNCTION_FLAGS_ASYNC_GENERATOR
            == ast::FUNCTION_FLAGS_ASYNC_GENERATOR
    }

    fn visit_await_expression(&mut self, node: ast::Node) -> ast::Node {
        let (expression, loc) = {
            let source = self.store_for(node);
            (source.expression(node), source.loc(node))
        };
        let expression = expression
            .and_then(|node| self.visit_node(Some(node)))
            .expect("await expression should have expression");
        let await_helper = self.emit_context.factory.new_await_helper(expression);
        let result = self
            .factory_mut()
            .new_yield_expression(None::<ast::Node>, await_helper);
        self.factory_mut().place_emit_synthetic_node(result, loc);
        self.emit_context.set_original(&result, &node);
        result
    }

    fn visit_yield_expression(&mut self, node: ast::Node) -> ast::Node {
        let (asterisk, expression, loc, node_store_id) = {
            let source = self.store_for(node);
            (
                source.asterisk_token(node),
                source.expression(node),
                source.loc(node),
                source.store_id(),
            )
        };
        if asterisk.is_some() {
            let expression = expression
                .and_then(|node| self.visit_node(Some(node)))
                .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
            let async_values = self
                .emit_context
                .factory
                .new_async_values_helper(expression);
            self.factory_mut()
                .place_emit_synthetic_node(async_values, loc);
            let async_delegator = self
                .emit_context
                .factory
                .new_async_delegator_helper(async_values);
            self.factory_mut()
                .place_emit_synthetic_node(async_delegator, loc);
            let asterisk = asterisk.map(|token| self.preserve_node(token));
            let inner_yield = if node_store_id == self.factory().store().store_id() {
                self.factory_mut()
                    .update_yield_expression(node, asterisk, async_delegator)
            } else {
                let source = self.source;
                self.factory_mut().update_yield_expression_from_store(
                    source,
                    node,
                    asterisk,
                    async_delegator,
                )
            };
            let awaited = self.emit_context.factory.new_await_helper(inner_yield);
            let result = self
                .factory_mut()
                .new_yield_expression(None::<ast::Node>, awaited);
            self.factory_mut().place_emit_synthetic_node(result, loc);
            self.emit_context.set_original(&result, &node);
            return result;
        }

        let expression = expression
            .and_then(|node| self.visit_node(Some(node)))
            .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
        let awaited = self.create_downlevel_await(expression);
        let result = self
            .factory_mut()
            .new_yield_expression(None::<ast::Node>, awaited);
        self.factory_mut().place_emit_synthetic_node(result, loc);
        self.emit_context.set_original(&result, &node);
        result
    }

    fn visit_return_statement(&mut self, node: ast::Node) -> ast::Node {
        let (expression, node_store_id) = {
            let source = self.store_for(node);
            (source.expression(node), source.store_id())
        };
        let expression = expression
            .and_then(|node| self.visit_node(Some(node)))
            .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
        let expression = self.create_downlevel_await(expression);
        if node_store_id == self.factory().store().store_id() {
            self.factory_mut().update_return_statement(node, expression)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_return_statement_from_store(source, node, expression)
        }
    }

    fn create_downlevel_await(&mut self, expression: ast::Node) -> ast::Node {
        if self.enclosing_function_flags & ast::FUNCTION_FLAGS_GENERATOR != 0 {
            let await_helper = self.emit_context.factory.new_await_helper(expression);
            return self
                .factory_mut()
                .new_yield_expression(None::<ast::Node>, await_helper);
        }
        self.factory_mut().new_await_expression(expression)
    }

    fn visit_labeled_statement(&mut self, node: ast::Node) -> ast::Node {
        let statement = self.unwrap_innermost_statement_of_label(node);
        let statement_source = self.store_for(statement);
        if statement_source.kind(statement) == ast::Kind::ForOfStatement
            && statement_source.await_modifier(statement).is_some()
        {
            return self.visit_for_of_statement(statement, Some(node));
        }
        let visited = self
            .visit_node(Some(statement))
            .unwrap_or_else(|| self.preserve_node(statement));
        self.restore_enclosing_label(&visited, Some(&node))
    }

    // unwrapInnermostStatementOfLabel follows LabeledStatement chains to find the innermost statement.
    fn unwrap_innermost_statement_of_label(&self, mut node: ast::Node) -> ast::Node {
        loop {
            let statement = self
                .store_for(node)
                .statement(node)
                .expect("labeled statement should have a statement");
            if self.store_for(statement).kind(statement) != ast::Kind::LabeledStatement {
                return statement;
            }
            node = statement;
        }
    }

    // visitForOfStatement visits a ForOfStatement and converts it into a ES2015-compatible ForOfStatement.
    fn visit_for_of_statement(
        &mut self,
        node: ast::Node,
        outermost_labeled_statement: Option<ast::Node>,
    ) -> ast::Node {
        let ancestor_facts = self.hierarchy_facts;
        let facts = enter_iteration_container(self.hierarchy_facts);
        self.with_hierarchy_facts(facts, |this| {
            if this.store_for(node).await_modifier(node).is_some() {
                this.transform_for_await_of_statement(
                    node,
                    outermost_labeled_statement,
                    ancestor_facts,
                )
            } else {
                let visited = this.generated_visit_each_child(&node);
                this.restore_enclosing_label(&visited, outermost_labeled_statement.as_ref())
            }
        })
    }

    fn convert_for_of_statement_head(
        &mut self,
        node: ast::Node,
        bound_value: ast::Node,
        non_user_code: ast::Node,
    ) -> ast::Node {
        let value = self.emit_context.factory.new_temp_variable();
        self.emit_context.add_variable_declaration(value);

        let iterator_value_expression = self
            .emit_context
            .factory
            .new_assignment_expression(value, bound_value);
        let iterator_value_statement = self
            .factory_mut()
            .new_expression_statement(iterator_value_expression);
        let (expression_loc, initializer, statement_node) = {
            let source = self.store_for(node);
            (
                source
                    .expression(node)
                    .map(|expression| source.loc(expression))
                    .unwrap_or_else(core::undefined_text_range),
                source
                    .initializer(node)
                    .expect("for-of statement should have an initializer"),
                source.statement(node),
            )
        };
        self.emit_context
            .set_source_map_range(&iterator_value_statement, expression_loc);

        let false_keyword = self
            .factory_mut()
            .new_keyword_expression(ast::Kind::FalseKeyword);
        let exit_non_user_code_expression = self
            .emit_context
            .factory
            .new_assignment_expression(non_user_code, false_keyword);
        let exit_non_user_code_statement = self
            .factory_mut()
            .new_expression_statement(exit_non_user_code_expression);
        self.emit_context
            .set_source_map_range(&exit_non_user_code_statement, expression_loc);

        let mut statements = vec![iterator_value_statement, exit_non_user_code_statement];
        let binding = self.emit_context.factory.create_for_of_binding_statement(
            self.source,
            &initializer,
            &value,
        );
        if let Some(binding) = self.visit_node(Some(binding)) {
            statements.push(binding);
        }

        let mut body_location = core::undefined_text_range();
        let mut statements_location = core::undefined_text_range();
        let statement = self.visit_embedded_statement(statement_node);
        if let Some(statement) = statement {
            if ast::is_block(self.store_for(statement), statement) {
                if let Some(source_statements) =
                    self.store_for(statement).source_statements(statement)
                {
                    statements.extend(source_statements.iter());
                    statements_location = source_statements.loc();
                }
                body_location = self.store_for(statement).loc(statement);
            } else {
                statements.push(statement);
            }
        }

        let statement_list =
            self.factory_mut()
                .new_node_list(statements_location, statements_location, statements);
        let block = self.factory_mut().new_block(statement_list, true);
        self.factory_mut()
            .place_emit_synthetic_node(block, body_location);
        block
    }

    fn transform_for_await_of_statement(
        &mut self,
        node: ast::Node,
        outermost_labeled_statement: Option<ast::Node>,
        ancestor_facts: ForAwaitHierarchyFacts,
    ) -> ast::Node {
        let (source_expression, source_expression_loc, node_loc) = {
            let source = self.store_for(node);
            let source_expression = source
                .expression(node)
                .expect("for-await-of statement should have an expression");
            (
                source_expression,
                source.loc(source_expression),
                source.loc(node),
            )
        };
        let expression = self
            .visit_node(Some(source_expression))
            .unwrap_or_else(|| self.preserve_node(source_expression));

        let iterator = if ast::is_identifier(self.store_for(expression), expression) {
            self.emit_context
                .factory
                .new_generated_name_for_factory_node(&expression)
        } else {
            self.emit_context.factory.new_temp_variable()
        };

        let result = if ast::is_identifier(self.store_for(expression), expression) {
            self.emit_context
                .factory
                .new_generated_name_for_factory_node(&iterator)
        } else {
            self.emit_context.factory.new_temp_variable()
        };

        let non_user_code = self.emit_context.factory.new_temp_variable();
        let done = self.emit_context.factory.new_temp_variable();
        self.emit_context.add_variable_declaration(done);
        let error_record = self.emit_context.factory.new_unique_name("e");
        let catch_variable = self
            .emit_context
            .factory
            .new_generated_name_for_factory_node(&error_record);
        let return_method = self.emit_context.factory.new_temp_variable();
        let call_values = self
            .emit_context
            .factory
            .new_async_values_helper(expression);
        self.factory_mut()
            .place_emit_synthetic_node(call_values, source_expression_loc);

        let next_name = self.factory_mut().new_identifier("next");
        let next_access = self.factory_mut().new_property_access_expression(
            iterator,
            None::<ast::Node>,
            next_name,
            ast::NodeFlags::NONE,
        );
        let empty_arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            Vec::<ast::Node>::new(),
        );
        let call_next = self.factory_mut().new_call_expression(
            next_access,
            None::<ast::Node>,
            None::<ast::NodeList>,
            empty_arguments,
            ast::NodeFlags::NONE,
        );

        let done_name = self.factory_mut().new_identifier("done");
        let get_done = self.factory_mut().new_property_access_expression(
            result,
            None::<ast::Node>,
            done_name,
            ast::NodeFlags::NONE,
        );
        let value_name = self.factory_mut().new_identifier("value");
        let get_value = self.factory_mut().new_property_access_expression(
            result,
            None::<ast::Node>,
            value_name,
            ast::NodeFlags::NONE,
        );
        let call_return =
            self.emit_context
                .factory
                .new_function_call_call(&return_method, Some(&iterator), &[]);

        self.emit_context.add_variable_declaration(error_record);
        self.emit_context.add_variable_declaration(return_method);

        // if we are enclosed in an outer loop ensure we reset 'errorRecord' per each iteration
        let initializer = if ancestor_facts.iteration_container {
            let void_zero = self.emit_context.factory.new_void_zero_expression();
            let reset_error_record = self
                .emit_context
                .factory
                .new_assignment_expression(error_record, void_zero);
            self.emit_context
                .factory
                .inline_expressions(&[reset_error_record, call_values])
        } else {
            Some(call_values)
        };

        // Build the for statement
        let iterator_decl = self.factory_mut().new_variable_declaration(
            iterator,
            None::<ast::Node>,
            None::<ast::Node>,
            initializer,
        );
        self.factory_mut()
            .place_emit_synthetic_node(iterator_decl, source_expression_loc);

        let true_keyword = self
            .factory_mut()
            .new_keyword_expression(ast::Kind::TrueKeyword);
        let non_user_code_decl = self.factory_mut().new_variable_declaration(
            non_user_code,
            None::<ast::Node>,
            None::<ast::Node>,
            true_keyword,
        );
        let result_decl = self.factory_mut().new_variable_declaration(
            result,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let var_declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![non_user_code_decl, iterator_decl, result_decl],
        );
        let var_decl_list = self
            .factory_mut()
            .new_variable_declaration_list(var_declarations, ast::NodeFlags::NONE);
        self.factory_mut()
            .place_emit_synthetic_node(var_decl_list, source_expression_loc);

        let awaited_next = self.create_downlevel_await(call_next);
        let assign_result = self
            .emit_context
            .factory
            .new_assignment_expression(result, awaited_next);
        let assign_done = self
            .emit_context
            .factory
            .new_assignment_expression(done, get_done);
        let not_done = self
            .factory_mut()
            .new_prefix_unary_expression(ast::Kind::ExclamationToken, done);
        let condition =
            self.emit_context
                .factory
                .inline_expressions(&[assign_result, assign_done, not_done]);

        let true_keyword = self
            .factory_mut()
            .new_keyword_expression(ast::Kind::TrueKeyword);
        let incrementor = self
            .emit_context
            .factory
            .new_assignment_expression(non_user_code, true_keyword);
        let statement = self.convert_for_of_statement_head(node, get_value, non_user_code);
        let for_statement =
            self.factory_mut()
                .new_for_statement(var_decl_list, condition, incrementor, statement);
        self.factory_mut()
            .place_emit_synthetic_node(for_statement, node_loc);
        self.emit_context
            .set_emit_flags(&for_statement, printer::EF_NO_TOKEN_TRAILING_SOURCE_MAPS);
        self.emit_context.set_original(&for_statement, &node);

        // Build the try/catch/finally
        let labeled_for_statement =
            self.restore_enclosing_label(&for_statement, outermost_labeled_statement.as_ref());
        let try_statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![labeled_for_statement],
        );
        let try_block = self.factory_mut().new_block(try_statements, true);

        // catch clause: { e_1 = { error: e_2 }; }
        let error_name = self.factory_mut().new_identifier("error");
        let error_property = self.factory_mut().new_property_assignment(
            None::<ast::ModifierList>,
            error_name,
            None::<ast::Node>,
            None::<ast::Node>,
            catch_variable,
        );
        let error_properties = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![error_property],
        );
        let error_object = self
            .factory_mut()
            .new_object_literal_expression(error_properties, false);
        let assign_error_record = self
            .emit_context
            .factory
            .new_assignment_expression(error_record, error_object);
        let catch_statement = self
            .factory_mut()
            .new_expression_statement(assign_error_record);
        let catch_statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![catch_statement],
        );
        let catch_body = self.factory_mut().new_block(catch_statements, false);
        self.emit_context
            .set_emit_flags(&catch_body, printer::EF_SINGLE_LINE);
        let catch_declaration = self.factory_mut().new_variable_declaration(
            catch_variable,
            None::<ast::Node>,
            None::<ast::Node>,
            None::<ast::Node>,
        );
        let catch_clause = self
            .factory_mut()
            .new_catch_clause(catch_declaration, catch_body);

        // finally block
        // inner try: if (!nonUserCode && !done && (returnMethod = iterator.return)) await returnMethod.call(iterator);
        let not_non_user_code = self
            .factory_mut()
            .new_prefix_unary_expression(ast::Kind::ExclamationToken, non_user_code);
        let not_done = self
            .factory_mut()
            .new_prefix_unary_expression(ast::Kind::ExclamationToken, done);
        let left_inner_if_condition = self
            .emit_context
            .factory
            .new_logical_and_expression(not_non_user_code, not_done);
        let return_name = self.factory_mut().new_identifier("return");
        let iterator_return = self.factory_mut().new_property_access_expression(
            iterator,
            None::<ast::Node>,
            return_name,
            ast::NodeFlags::NONE,
        );
        let assign_return_method = self
            .emit_context
            .factory
            .new_assignment_expression(return_method, iterator_return);
        let inner_if_condition = self
            .emit_context
            .factory
            .new_logical_and_expression(left_inner_if_condition, assign_return_method);
        let await_return = self.create_downlevel_await(call_return);
        let inner_then_statement = self.factory_mut().new_expression_statement(await_return);
        let inner_if_statement = self.factory_mut().new_if_statement(
            inner_if_condition,
            inner_then_statement,
            None::<ast::Node>,
        );
        self.emit_context
            .set_emit_flags(&inner_if_statement, printer::EF_SINGLE_LINE);

        let inner_try_statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![inner_if_statement],
        );
        let inner_try_block = self.factory_mut().new_block(inner_try_statements, false);

        // inner finally: if (errorRecord) throw errorRecord.error;
        let error_name = self.factory_mut().new_identifier("error");
        let error_access = self.factory_mut().new_property_access_expression(
            error_record,
            None::<ast::Node>,
            error_name,
            ast::NodeFlags::NONE,
        );
        let throw_error = self.factory_mut().new_throw_statement(error_access);
        let inner_finally_if =
            self.factory_mut()
                .new_if_statement(error_record, throw_error, None::<ast::Node>);
        self.emit_context
            .set_emit_flags(&inner_finally_if, printer::EF_SINGLE_LINE);
        let inner_finally_statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![inner_finally_if],
        );
        let inner_finally_block = self
            .factory_mut()
            .new_block(inner_finally_statements, false);
        self.emit_context
            .set_emit_flags(&inner_finally_block, printer::EF_SINGLE_LINE);

        let inner_try_statement = self.factory_mut().new_try_statement(
            inner_try_block,
            None::<ast::Node>,
            inner_finally_block,
        );
        let finally_statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![inner_try_statement],
        );
        let finally_block = self.factory_mut().new_block(finally_statements, true);

        self.factory_mut()
            .new_try_statement(try_block, catch_clause, finally_block)
    }

    fn visit_constructor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let flags = ast::get_function_flags(source, Some(node));
        self.with_enclosing_function_flags(flags, |this| {
            let (modifiers_input, parameters_input, body_node) = {
                let source = this.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    ast::SourceNodeListInput::from_source(
                        source
                            .source_parameters(node)
                            .expect("constructor parameters should exist"),
                    ),
                    source.body(node),
                )
            };
            let modifiers = this.visit_modifiers_input(modifiers_input);
            let parameters = this
                .visit_parameters_input(Some(parameters_input))
                .expect("constructor parameters should exist");
            let body = this.visit_function_body(body_node);
            if this.is_factory_node(node) {
                this.factory_mut().update_constructor_declaration(
                    node,
                    modifiers,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            } else {
                let source = this.source;
                this.factory_mut()
                    .update_constructor_declaration_from_store(
                        source,
                        node,
                        modifiers,
                        None::<ast::NodeList>,
                        parameters,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        body,
                    )
            }
        })
    }

    fn visit_get_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let flags = ast::get_function_flags(source, Some(node));
        self.with_enclosing_function_flags(flags, |this| {
            let (modifiers_input, name_node, parameters_input, body_node) = {
                let source = this.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.name(node),
                    ast::SourceNodeListInput::from_source(
                        source
                            .source_parameters(node)
                            .expect("accessor parameters should exist"),
                    ),
                    source.body(node),
                )
            };
            let modifiers = this.visit_modifiers_input(modifiers_input);
            let name = name_node.and_then(|node| this.visit_node(Some(node)));
            let parameters = this
                .visit_parameters_input(Some(parameters_input))
                .expect("accessor parameters should exist");
            let body = this.visit_function_body(body_node);
            if this.is_factory_node(node) {
                this.factory_mut().update_get_accessor_declaration(
                    node,
                    modifiers,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            } else {
                let source = this.source;
                this.factory_mut()
                    .update_get_accessor_declaration_from_store(
                        source,
                        node,
                        modifiers,
                        name,
                        None::<ast::NodeList>,
                        parameters,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        body,
                    )
            }
        })
    }

    fn visit_set_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let flags = ast::get_function_flags(source, Some(node));
        self.with_enclosing_function_flags(flags, |this| {
            let (modifiers_input, name_node, parameters_input, body_node) = {
                let source = this.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.name(node),
                    ast::SourceNodeListInput::from_source(
                        source
                            .source_parameters(node)
                            .expect("accessor parameters should exist"),
                    ),
                    source.body(node),
                )
            };
            let modifiers = this.visit_modifiers_input(modifiers_input);
            let name = name_node.and_then(|node| this.visit_node(Some(node)));
            let parameters = this
                .visit_parameters_input(Some(parameters_input))
                .expect("accessor parameters should exist");
            let body = this.visit_function_body(body_node);
            if this.is_factory_node(node) {
                this.factory_mut().update_set_accessor_declaration(
                    node,
                    modifiers,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            } else {
                let source = this.source;
                this.factory_mut()
                    .update_set_accessor_declaration_from_store(
                        source,
                        node,
                        modifiers,
                        name,
                        None::<ast::NodeList>,
                        parameters,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        body,
                    )
            }
        })
    }

    fn visit_method_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let flags = ast::get_function_flags(source, Some(node));
        self.with_enclosing_function_flags(flags, |this| {
            let (modifiers_input, asterisk_node, parameters_input, body_node, name_node) = {
                let source = this.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.asterisk_token(node),
                    ast::SourceNodeListInput::from_source(
                        source
                            .source_parameters(node)
                            .expect("method parameters should exist"),
                    ),
                    source.body(node),
                    source.name(node),
                )
            };
            let async_generator = this.in_async_generator();
            let modifiers = if flags & ast::FUNCTION_FLAGS_GENERATOR != 0 {
                this.visit_modifiers_input_no_async(modifiers_input)
            } else {
                this.visit_modifiers_input(modifiers_input)
            };
            let asterisk = if flags & ast::FUNCTION_FLAGS_ASYNC != 0 {
                None
            } else {
                asterisk_node.map(|node| this.preserve_node(node))
            };
            let parameters = if async_generator {
                this.transform_async_generator_function_parameter_list(node)
            } else {
                this.visit_parameters_input(Some(parameters_input))
                    .expect("method parameters should exist")
            };
            let body = if async_generator {
                Some(this.transform_async_generator_function_body(node))
            } else {
                this.visit_function_body(body_node)
            };
            let name = name_node.and_then(|node| this.visit_node(Some(node)));
            if this.is_factory_node(node) {
                this.factory_mut().update_method_declaration(
                    node,
                    modifiers,
                    asterisk,
                    name,
                    None::<ast::Node>,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            } else {
                let source = this.source;
                this.factory_mut().update_method_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    asterisk,
                    name,
                    None::<ast::Node>,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            }
        })
    }

    fn visit_function_declaration(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let flags = ast::get_function_flags(source, Some(node));
        self.with_enclosing_function_flags(flags, |this| {
            let (modifiers_input, asterisk_node, parameters_input, body_node, name_node) = {
                let source = this.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.asterisk_token(node),
                    ast::SourceNodeListInput::from_source(
                        source
                            .source_parameters(node)
                            .expect("function parameters should exist"),
                    ),
                    source.body(node),
                    source.name(node),
                )
            };
            let async_generator = this.in_async_generator();
            let modifiers = if flags & ast::FUNCTION_FLAGS_GENERATOR != 0 {
                this.visit_modifiers_input_no_async(modifiers_input)
            } else {
                this.visit_modifiers_input(modifiers_input)
            };
            let asterisk = if flags & ast::FUNCTION_FLAGS_ASYNC != 0 {
                None
            } else {
                asterisk_node.map(|node| this.preserve_node(node))
            };
            let parameters = if async_generator {
                this.transform_async_generator_function_parameter_list(node)
            } else {
                this.visit_parameters_input(Some(parameters_input))
                    .expect("function parameters should exist")
            };
            let body = if async_generator {
                Some(this.transform_async_generator_function_body(node))
            } else {
                this.visit_function_body(body_node)
            };
            let name = name_node.and_then(|node| this.visit_node(Some(node)));
            if this.is_factory_node(node) {
                this.factory_mut().update_function_declaration(
                    node,
                    modifiers,
                    asterisk,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            } else {
                let source = this.source;
                this.factory_mut().update_function_declaration_from_store(
                    source,
                    node,
                    modifiers,
                    asterisk,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            }
        })
    }

    fn visit_function_expression(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let flags = ast::get_function_flags(source, Some(node));
        self.with_enclosing_function_flags(flags, |this| {
            let (modifiers_input, asterisk_node, parameters_input, body_node, name_node) = {
                let source = this.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    source.asterisk_token(node),
                    ast::SourceNodeListInput::from_source(
                        source
                            .source_parameters(node)
                            .expect("function parameters should exist"),
                    ),
                    source.body(node),
                    source.name(node),
                )
            };
            let async_generator = this.in_async_generator();
            let modifiers = if flags & ast::FUNCTION_FLAGS_GENERATOR != 0 {
                this.visit_modifiers_input_no_async(modifiers_input)
            } else {
                this.visit_modifiers_input(modifiers_input)
            };
            let asterisk = if flags & ast::FUNCTION_FLAGS_ASYNC != 0 {
                None
            } else {
                asterisk_node.map(|node| this.preserve_node(node))
            };
            let parameters = if async_generator {
                this.transform_async_generator_function_parameter_list(node)
            } else {
                this.visit_parameters_input(Some(parameters_input))
                    .expect("function parameters should exist")
            };
            let body = if async_generator {
                Some(this.transform_async_generator_function_body(node))
            } else {
                this.visit_function_body(body_node)
            };
            let name = name_node.and_then(|node| this.visit_node(Some(node)));
            if this.is_factory_node(node) {
                this.factory_mut().update_function_expression(
                    node,
                    modifiers,
                    asterisk,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            } else {
                let source = this.source;
                this.factory_mut().update_function_expression_from_store(
                    source,
                    node,
                    modifiers,
                    asterisk,
                    name,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    body,
                )
            }
        })
    }

    fn visit_arrow_function(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let flags = ast::get_function_flags(source, Some(node));
        self.with_enclosing_function_flags(flags, |this| {
            let (modifiers_input, parameters_input, body_node, equals_node) = {
                let source = this.store_for(node);
                (
                    source
                        .source_modifiers(node)
                        .map(ast::SourceModifierListInput::from_source),
                    ast::SourceNodeListInput::from_source(
                        source
                            .source_parameters(node)
                            .expect("arrow parameters should exist"),
                    ),
                    source.body(node),
                    source.equals_greater_than_token(node),
                )
            };
            let modifiers = this.visit_modifiers_input(modifiers_input);
            let parameters = this
                .visit_parameters_input(Some(parameters_input))
                .expect("arrow parameters should exist");
            let body = this.visit_function_body(body_node);
            let equals = equals_node.map(|node| this.preserve_node(node));
            if this.is_factory_node(node) {
                this.factory_mut().update_arrow_function(
                    node,
                    modifiers,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    equals,
                    body,
                )
            } else {
                let source = this.source;
                this.factory_mut().update_arrow_function_from_store(
                    source,
                    node,
                    modifiers,
                    None::<ast::NodeList>,
                    parameters,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    equals,
                    body,
                )
            }
        })
    }

    fn transform_async_generator_function_parameter_list(
        &mut self,
        node: ast::Node,
    ) -> ast::NodeList {
        if self.is_simple_parameter_list(node) {
            return self
                .visit_parameters_input(self.source_parameters_input(node))
                .expect("async generator parameters should exist");
        }
        let (parameters, loc, range) = {
            let parameters = self
                .store_for(node)
                .source_parameters(node)
                .expect("async generator parameters should exist");
            (
                parameters.iter().collect::<Vec<_>>(),
                parameters.loc(),
                parameters.range(),
            )
        };
        let mut new_parameters = Vec::new();
        for parameter in parameters {
            let (initializer, dot_dot_dot, source_name) = {
                let parameter_source = self.store_for(parameter);
                (
                    parameter_source.initializer(parameter),
                    parameter_source.dot_dot_dot_token(parameter),
                    parameter_source
                        .name(parameter)
                        .expect("parameter should have name"),
                )
            };
            if initializer.is_some() || dot_dot_dot.is_some() {
                break;
            }
            let name = self.new_generated_name_for_node_ex(
                source_name,
                printer::AutoGenerateOptions {
                    flags: printer::GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                    ..Default::default()
                },
            );
            let new_parameter = self.factory_mut().new_parameter_declaration(
                None::<ast::ModifierList>,
                None::<ast::Node>,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                None::<ast::Node>,
            );
            new_parameters.push(new_parameter);
        }
        self.factory_mut().new_node_list(loc, range, new_parameters)
    }

    fn transform_async_generator_function_body(&mut self, node: ast::Node) -> ast::Node {
        let body_node = self
            .store_for(node)
            .body(node)
            .expect("async generator function should have a body");
        let inner_parameters = if self.is_simple_parameter_list(node) {
            None
        } else {
            self.visit_parameters_input(self.source_parameters_input(node))
        };

        let saved_super_state = self.super_access_state.clone();
        let saved_super_binding = self.super_binding;
        let saved_super_index_binding = self.super_index_binding;
        self.super_access_state = Some(self.new_super_state_for_body(Some(body_node)));
        self.super_binding = Some(self.new_super_binding("_super"));
        self.super_index_binding = Some(self.new_super_binding("_superIndex"));

        let mut async_body = self
            .with_super_substitution(|this| this.visit_block_as_async_generator_body(body_node));
        let statements = self
            .factory()
            .store()
            .source_statements(async_body)
            .expect("async generator body should be a block")
            .iter()
            .collect::<Vec<_>>();
        let statements = self
            .emit_context
            .end_and_merge_variable_environment(self.source, &statements);
        let statements = self.new_node_list_like_block(async_body, statements);
        let multi_line = self
            .store_for(async_body)
            .multi_line(async_body)
            .unwrap_or(true);
        async_body = self
            .factory_mut()
            .update_block(async_body, statements, multi_line);

        let emit_super_helpers = self.super_access_state.as_ref().is_some_and(|state| {
            !state.captured_super_properties.is_empty() || state.has_super_element_access
        });
        self.emit_context.start_variable_environment();
        if emit_super_helpers
            && self
                .super_access_state
                .as_ref()
                .is_some_and(|state| !state.captured_super_properties.is_empty())
        {
            let statement = self.create_super_access_variable_statement();
            self.emit_context.add_initialization_statement(statement);
        }

        let inner_params = inner_parameters.unwrap_or_else(|| {
            self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            )
        });
        let name = self
            .store_for(node)
            .name(node)
            .map(|name| self.new_generated_name_for_node_ex(name, Default::default()));
        let asterisk = self.factory_mut().new_token(ast::Kind::AsteriskToken);
        let generator_func = self.factory_mut().new_function_expression(
            None::<ast::ModifierList>,
            asterisk,
            name,
            None::<ast::NodeList>,
            inner_params,
            None::<ast::Node>,
            None::<ast::Node>,
            async_body,
        );
        let async_generator = self
            .emit_context
            .factory
            .new_async_generator_helper(generator_func, self.hierarchy_facts.has_lexical_this);
        let return_statement = self.factory_mut().new_return_statement(async_generator);

        let mut outer_statements = self.emit_context.end_variable_environment();
        outer_statements.push(return_statement);
        let outer_statements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            outer_statements,
        );
        let body_store = self.store_for(body_node);
        let multi_line = body_store.multi_line(body_node).unwrap_or(true);
        let body_store_id = body_store.store_id();
        let block = if body_store_id == self.factory().store().store_id() {
            self.factory_mut()
                .update_block(body_node, outer_statements, multi_line)
        } else {
            let source = self.source;
            self.factory_mut().update_block_from_store(
                source,
                body_node,
                outer_statements,
                multi_line,
            )
        };

        if emit_super_helpers
            && self
                .super_access_state
                .as_ref()
                .is_some_and(|state| state.has_super_element_access)
        {
            if self
                .super_access_state
                .as_ref()
                .is_some_and(|state| state.has_super_property_assignment)
            {
                self.emit_context.add_emit_helper(
                    &block,
                    std::slice::from_ref(&printer::ADVANCED_ASYNC_SUPER_HELPER),
                );
            } else {
                self.emit_context
                    .add_emit_helper(&block, std::slice::from_ref(&printer::ASYNC_SUPER_HELPER));
            }
        }

        self.super_access_state = saved_super_state;
        self.super_binding = saved_super_binding;
        self.super_index_binding = saved_super_index_binding;
        block
    }

    fn visit_block_as_async_generator_body(&mut self, body: ast::Node) -> ast::Node {
        if ast::is_block(self.store_for(body), body) {
            let (statements_input, multi_line, body_store_id) = {
                let source = self.store_for(body);
                (
                    source
                        .source_statements(body)
                        .map(ast::SourceNodeListInput::from_source),
                    source.multi_line(body).unwrap_or(true),
                    source.store_id(),
                )
            };
            let statements = self
                .visit_nodes_input(statements_input)
                .expect("block statements should exist");
            return if body_store_id == self.factory().store().store_id() {
                self.factory_mut()
                    .update_block(body, statements, multi_line)
            } else {
                let source = self.source;
                self.factory_mut()
                    .update_block_from_store(source, body, statements, multi_line)
            };
        }
        let visited = self
            .visit_node(Some(body))
            .unwrap_or_else(|| self.preserve_node(body));
        let return_statement = self.factory_mut().new_return_statement(visited);
        let loc = self.store_for(body).loc(body);
        self.factory_mut()
            .place_emit_synthetic_node(return_statement, loc);
        let statements = self
            .factory_mut()
            .new_node_list(loc, loc, vec![return_statement]);
        let block = self.factory_mut().new_block(statements, false);
        self.factory_mut().place_emit_synthetic_node(block, loc);
        block
    }

    fn is_simple_parameter_list(&self, node: ast::Node) -> bool {
        self.store_for(node)
            .source_parameters(node)
            .is_none_or(|parameters| {
                parameters.iter().all(|parameter| {
                    let parameter_source = self.store_for(parameter);
                    parameter_source.initializer(parameter).is_none()
                        && parameter_source
                            .name(parameter)
                            .is_some_and(|name| ast::is_identifier(self.store_for(name), name))
                })
            })
    }

    fn visit_modifiers_input_no_async(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        let modifier_nodes = modifiers.nodes();
        let mut visited = Vec::with_capacity(modifier_nodes.len());
        let mut changed = false;
        for node in modifier_nodes.iter() {
            if self.store_for(*node).kind(*node) == ast::Kind::AsyncKeyword {
                changed = true;
                continue;
            }
            let result = self.visit(&node);
            self.append_visited_node(*node, result, &mut visited, &mut changed);
        }
        if changed {
            Some(self.factory_mut().new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                visited,
                ast::ModifierFlags::NONE,
            ))
        } else {
            Some(self.import_state.preserve_source_modifier_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &modifiers,
            ))
        }
    }

    fn new_super_state_for_body(&mut self, body: Option<ast::Node>) -> SuperAccessState {
        let mut state = SuperAccessState::default();
        if let Some(body) = body {
            self.collect_super_accesses(body, &mut state);
        }
        state
    }

    fn new_super_binding(&mut self, name: &str) -> ast::Node {
        self.emit_context.factory.new_unique_name_ex(
            name,
            printer::AutoGenerateOptions {
                flags: printer::GeneratedIdentifierFlags::OPTIMISTIC
                    | printer::GeneratedIdentifierFlags::FILE_LEVEL
                    | printer::GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                ..Default::default()
            },
        )
    }

    fn collect_super_accesses(&self, node: ast::Node, state: &mut SuperAccessState) {
        self.track_super_access(node, state);
        let source = self.store_for(node);
        match source.kind(node) {
            ast::Kind::FunctionExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::Constructor
            | ast::Kind::ClassDeclaration
            | ast::Kind::ClassExpression => return,
            _ => {}
        }
        let _ = source.for_each_present_child(node, |child| {
            self.collect_super_accesses(child, state);
            std::ops::ControlFlow::Continue(())
        });
    }

    fn track_super_access(&self, node: ast::Node, state: &mut SuperAccessState) {
        let source = self.store_for(node);
        let expression_is_super = self.expression_is_super(node);
        let property_name = source
            .name(node)
            .filter(|_| expression_is_super)
            .map(|name| self.store_for(name).text(name));
        let assignment_target_contains_super_property = match source.kind(node) {
            ast::Kind::BinaryExpression => {
                source.operator_token(node).is_some_and(|operator| {
                    ast::is_assignment_operator(self.store_for(operator).kind(operator))
                }) && source
                    .left(node)
                    .is_some_and(|left| self.assignment_target_contains_super_property(left))
            }
            ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => self
                .store_for(node)
                .operand(node)
                .is_some_and(|operand| self.assignment_target_contains_super_property(operand)),
            _ => false,
        };
        utilities::track_super_access(
            state,
            source.kind(node),
            expression_is_super,
            property_name.as_deref(),
            assignment_target_contains_super_property,
            self.is_update_expression(node),
        );
    }

    fn expression_is_super(&self, node: ast::Node) -> bool {
        self.store_for(node)
            .expression(node)
            .is_some_and(|expression| {
                self.store_for(expression).kind(expression) == ast::Kind::SuperKeyword
            })
    }

    fn assignment_target_contains_super_property(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        match source.kind(node) {
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                self.expression_is_super(node)
            }
            ast::Kind::ParenthesizedExpression | ast::Kind::SpreadElement => {
                source.expression(node).is_some_and(|expression| {
                    self.assignment_target_contains_super_property(expression)
                })
            }
            ast::Kind::ArrayLiteralExpression => {
                source.source_elements(node).is_some_and(|elements| {
                    elements
                        .iter()
                        .any(|element| self.assignment_target_contains_super_property(element))
                })
            }
            ast::Kind::ObjectLiteralExpression => {
                source.source_properties(node).is_some_and(|properties| {
                    properties.iter().any(|property| {
                        let property_source = self.store_for(property);
                        match property_source.kind(property) {
                            ast::Kind::PropertyAssignment => property_source
                                .initializer(property)
                                .is_some_and(|initializer| {
                                    self.assignment_target_contains_super_property(initializer)
                                }),
                            ast::Kind::ShorthandPropertyAssignment => {
                                property_source.name(property).is_some_and(|name| {
                                    self.assignment_target_contains_super_property(name)
                                })
                            }
                            ast::Kind::SpreadAssignment => property_source
                                .expression(property)
                                .is_some_and(|expression| {
                                    self.assignment_target_contains_super_property(expression)
                                }),
                            _ => false,
                        }
                    })
                })
            }
            _ => false,
        }
    }

    fn is_update_expression(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        match source.kind(node) {
            ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => {
                source.operator(node).is_some_and(|operator| {
                    matches!(
                        operator,
                        ast::Kind::PlusPlusToken | ast::Kind::MinusMinusToken
                    )
                })
            }
            _ => false,
        }
    }

    fn try_substitute_super_access(&mut self, node: ast::Node) -> Option<ast::Node> {
        self.super_access_state.as_ref()?;
        let source = self.store_for(node);
        match source.kind(node) {
            ast::Kind::CallExpression => {
                let expression = source.expression(node)?;
                if (ast::is_property_access_expression(self.store_for(expression), expression)
                    || ast::is_element_access_expression(self.store_for(expression), expression))
                    && self.expression_is_super(expression)
                {
                    return Some(
                        self.substitute_call_expression_with_super_access(node, expression),
                    );
                }
                None
            }
            ast::Kind::PropertyAccessExpression if self.expression_is_super(node) => {
                let super_binding = self
                    .super_binding
                    .expect("super binding should exist while substituting super access");
                let name = source.name(node).map(|name| self.preserve_node(name));
                Some(self.factory_mut().new_property_access_expression(
                    super_binding,
                    None::<ast::Node>,
                    name,
                    ast::NodeFlags::NONE,
                ))
            }
            ast::Kind::ElementAccessExpression if self.expression_is_super(node) => {
                let argument = source
                    .argument_expression(node)
                    .expect("element access should have an argument");
                Some(self.create_super_element_access_in_async_method(argument))
            }
            _ => None,
        }
    }

    fn substitute_call_expression_with_super_access(
        &mut self,
        call: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        let expression_source = self.store_for(expression);
        let target = if ast::is_property_access_expression(expression_source, expression) {
            let super_binding = self
                .super_binding
                .expect("super binding should exist while substituting super access");
            let name = self
                .store_for(expression)
                .name(expression)
                .map(|name| self.preserve_node(name));
            self.factory_mut().new_property_access_expression(
                super_binding,
                None::<ast::Node>,
                name,
                ast::NodeFlags::NONE,
            )
        } else if ast::is_element_access_expression(expression_source, expression) {
            let argument = self
                .store_for(expression)
                .argument_expression(expression)
                .expect("element access should have an argument");
            self.create_super_element_access_in_async_method(argument)
        } else {
            return self.generated_visit_each_child(&call);
        };

        let call_name = self.factory_mut().new_identifier("call");
        let call_target = self.factory_mut().new_property_access_expression(
            target,
            None::<ast::Node>,
            call_name,
            ast::NodeFlags::NONE,
        );
        let mut arguments = vec![self.emit_context.factory.new_this_expression()];
        let source_arguments = self
            .store_for(call)
            .source_arguments(call)
            .map(|arguments| arguments.iter().collect::<Vec<_>>());
        if let Some(source_arguments) = source_arguments {
            for argument in source_arguments {
                if let Some(argument) = self.visit_node(Some(argument)) {
                    arguments.push(argument);
                }
            }
        }
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            arguments,
        );
        let result = self.factory_mut().new_call_expression(
            call_target,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        );
        let loc = self.store_for(call).loc(call);
        self.factory_mut().place_emit_synthetic_node(result, loc);
        result
    }

    fn create_super_element_access_in_async_method(
        &mut self,
        argument_expression: ast::Node,
    ) -> ast::Node {
        let super_index_binding = self
            .super_index_binding
            .expect("super index binding should exist while substituting super access");
        let argument_expression = self
            .visit_node(Some(argument_expression))
            .unwrap_or_else(|| self.preserve_node(argument_expression));
        let arguments = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![argument_expression],
        );
        let super_index_call = self.factory_mut().new_call_expression(
            super_index_binding,
            None::<ast::Node>,
            None::<ast::NodeList>,
            arguments,
            ast::NodeFlags::NONE,
        );
        if self
            .super_access_state
            .as_ref()
            .is_some_and(|state| state.has_super_property_assignment)
        {
            let value = self.factory_mut().new_identifier("value");
            return self.factory_mut().new_property_access_expression(
                super_index_call,
                None::<ast::Node>,
                value,
                ast::NodeFlags::NONE,
            );
        }
        super_index_call
    }

    fn create_super_access_variable_statement(&mut self) -> ast::Node {
        let super_binding = self
            .super_binding
            .expect("super binding should exist while creating super access variable");
        let has_super_property_assignment = self
            .super_access_state
            .as_ref()
            .is_some_and(|state| state.has_super_property_assignment);
        let captured = self
            .super_access_state
            .as_ref()
            .map(|state| state.captured_super_properties.clone())
            .unwrap_or_default();
        let mut accessors = Vec::new();

        for name in captured {
            let mut descriptor_properties = Vec::new();

            let super_keyword = self
                .factory_mut()
                .new_keyword_expression(ast::Kind::SuperKeyword);
            let property_name = self.factory_mut().new_identifier(&name);
            let getter_body = self.factory_mut().new_property_access_expression(
                super_keyword,
                None::<ast::Node>,
                property_name,
                ast::NodeFlags::NONE,
            );
            let getter_params = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            let equals = self
                .factory_mut()
                .new_token(ast::Kind::EqualsGreaterThanToken);
            let getter_arrow = self.factory_mut().new_arrow_function(
                None::<ast::ModifierList>,
                None::<ast::NodeList>,
                getter_params,
                None::<ast::Node>,
                None::<ast::Node>,
                equals,
                getter_body,
            );
            let get_name = self.factory_mut().new_identifier("get");
            let getter = self.factory_mut().new_property_assignment(
                None::<ast::ModifierList>,
                get_name,
                None::<ast::Node>,
                None::<ast::Node>,
                getter_arrow,
            );
            descriptor_properties.push(getter);

            if has_super_property_assignment {
                let v_name = self.factory_mut().new_identifier("v");
                let v_param = self.factory_mut().new_parameter_declaration(
                    None::<ast::ModifierList>,
                    None::<ast::Node>,
                    v_name,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    None::<ast::Node>,
                );
                let super_keyword = self
                    .factory_mut()
                    .new_keyword_expression(ast::Kind::SuperKeyword);
                let property_name = self.factory_mut().new_identifier(&name);
                let super_prop = self.factory_mut().new_property_access_expression(
                    super_keyword,
                    None::<ast::Node>,
                    property_name,
                    ast::NodeFlags::NONE,
                );
                let v_ref = self.factory_mut().new_identifier("v");
                let assign_expr = self
                    .emit_context
                    .factory
                    .new_assignment_expression(super_prop, v_ref);
                let setter_params = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    vec![v_param],
                );
                let equals = self
                    .factory_mut()
                    .new_token(ast::Kind::EqualsGreaterThanToken);
                let setter_arrow = self.factory_mut().new_arrow_function(
                    None::<ast::ModifierList>,
                    None::<ast::NodeList>,
                    setter_params,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    equals,
                    assign_expr,
                );
                let set_name = self.factory_mut().new_identifier("set");
                let setter = self.factory_mut().new_property_assignment(
                    None::<ast::ModifierList>,
                    set_name,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    setter_arrow,
                );
                descriptor_properties.push(setter);
            }

            let descriptor_properties = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                descriptor_properties,
            );
            let descriptor = self
                .factory_mut()
                .new_object_literal_expression(descriptor_properties, false);
            let accessor_name = self.factory_mut().new_identifier(&name);
            let accessor = self.factory_mut().new_property_assignment(
                None::<ast::ModifierList>,
                accessor_name,
                None::<ast::Node>,
                None::<ast::Node>,
                descriptor,
            );
            accessors.push(accessor);
        }

        let descriptors = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            accessors,
        );
        let descriptors_object = self
            .factory_mut()
            .new_object_literal_expression(descriptors, true);
        let object_name = self.factory_mut().new_identifier("Object");
        let create_name = self.factory_mut().new_identifier("create");
        let object_create = self.factory_mut().new_property_access_expression(
            object_name,
            None::<ast::Node>,
            create_name,
            ast::NodeFlags::NONE,
        );
        let null_keyword = self
            .factory_mut()
            .new_keyword_expression(ast::Kind::NullKeyword);
        let call_args = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![null_keyword, descriptors_object],
        );
        let object_create_call = self.factory_mut().new_call_expression(
            object_create,
            None::<ast::Node>,
            None::<ast::NodeList>,
            call_args,
            ast::NodeFlags::NONE,
        );
        let declaration = self.factory_mut().new_variable_declaration(
            super_binding,
            None::<ast::Node>,
            None::<ast::Node>,
            object_create_call,
        );
        let declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![declaration],
        );
        let declaration_list = self
            .factory_mut()
            .new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        self.factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, declaration_list)
    }

    fn new_node_list_like_block(
        &mut self,
        block: ast::Node,
        statements: Vec<ast::Node>,
    ) -> ast::NodeList {
        if let Some(source_statements) = self.store_for(block).source_statements(block) {
            let view = source_statements;
            let loc = view.loc();
            let range = view.range();
            self.factory_mut().new_node_list(loc, range, statements)
        } else {
            self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                statements,
            )
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

impl<'source> ast::AstVisitEachChildRuntime<'source> for ForAwaitRuntime<'_, 'source> {
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
        let modifier_nodes = modifiers.nodes();
        let mut visited = Vec::with_capacity(modifier_nodes.len());
        let mut changed = false;
        for node in modifier_nodes.iter() {
            let result = self.visit(&node);
            self.append_visited_node(*node, result, &mut visited, &mut changed);
        }
        if changed {
            Some(self.factory_mut().new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                visited,
                ast::ModifierFlags::NONE,
            ))
        } else {
            Some(self.import_state.preserve_source_modifier_list_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &modifiers,
            ))
        }
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
        let source_nodes = nodes.clone();
        let mut visited = Vec::with_capacity(source_nodes.iter().len());
        let mut changed = false;
        for node in source_nodes.iter() {
            let result = node.and_then(|node| self.visit_node(Some(node)));
            match (node, result) {
                (Some(original), Some(result))
                    if self.preserved_source_node_matches(Some(original), Some(result)) =>
                {
                    visited.push(Some(self.preserve_node(original)));
                }
                (_, Some(result)) => {
                    changed = true;
                    visited.push(Some(self.preserve_node(result)));
                }
                (None, None) => visited.push(None),
                (Some(_), None) => {
                    changed = true;
                    visited.push(None);
                }
            }
        }
        if changed {
            Some(self.factory_mut().new_raw_node_slice(visited))
        } else {
            Some(self.import_state.preserve_source_raw_node_slice_input(
                self.source,
                &mut self.emit_context.factory.node_factory,
                &nodes,
            ))
        }
    }
}

impl<'source> ast::AstGeneratedVisitEachChild<'source> for ForAwaitRuntime<'_, 'source> {}
