use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalChainAction {
    Keep,
    VisitChildren,
    VisitCallExpression,
    VisitOptionalExpression,
    VisitDeleteExpression,
    CaptureParenthesizedOptionalCall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalChainSegmentKind {
    PropertyAccess,
    ElementAccess,
    Call,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalChainConditionalResult {
    VoidZeroOrRightExpression,
    TrueOrDeleteRightExpression,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OptionalChainFacts {
    pub subtree_contains_optional_chaining: bool,
    pub node_is_optional_chain: bool,
    pub expression_is_parenthesized_optional_chain: bool,
    pub capture_this_arg: bool,
    pub is_delete: bool,
    pub expression_is_simple_copiable: bool,
    pub left_is_synthetic_reference: bool,
    pub first_chain_segment_is_call: bool,
}

pub fn optional_chain_action_for_kind(
    kind: ast::Kind,
    facts: OptionalChainFacts,
) -> OptionalChainAction {
    if !facts.subtree_contains_optional_chaining {
        return OptionalChainAction::Keep;
    }

    match kind {
        ast::Kind::CallExpression if facts.node_is_optional_chain => {
            OptionalChainAction::VisitOptionalExpression
        }
        ast::Kind::CallExpression if facts.expression_is_parenthesized_optional_chain => {
            OptionalChainAction::CaptureParenthesizedOptionalCall
        }
        ast::Kind::CallExpression => OptionalChainAction::VisitCallExpression,
        ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression
            if facts.node_is_optional_chain =>
        {
            OptionalChainAction::VisitOptionalExpression
        }
        ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
            OptionalChainAction::VisitChildren
        }
        ast::Kind::DeleteExpression => OptionalChainAction::VisitDeleteExpression,
        _ => OptionalChainAction::VisitChildren,
    }
}

pub fn optional_chain_conditional_result(is_delete: bool) -> OptionalChainConditionalResult {
    if is_delete {
        OptionalChainConditionalResult::TrueOrDeleteRightExpression
    } else {
        OptionalChainConditionalResult::VoidZeroOrRightExpression
    }
}

pub fn should_capture_this_arg(
    capture_this_arg: bool,
    expression_is_simple_copiable: bool,
) -> bool {
    capture_this_arg && !expression_is_simple_copiable
}

pub fn should_capture_left_expression(expression_is_simple_copiable: bool) -> bool {
    !expression_is_simple_copiable
}

pub fn first_chain_visit_captures_this(first_chain_segment_is_call: bool) -> bool {
    first_chain_segment_is_call
}

pub fn optional_call_uses_function_call_call(left_is_synthetic_reference: bool) -> bool {
    left_is_synthetic_reference
}

pub fn optional_chain_not_null_condition_is_inverted() -> bool {
    true
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    let mut runtime = OptionalChainRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
    };
    runtime.visit_node(Some(root)).unwrap_or(root)
}

struct FlattenResult {
    expression: ast::Node,
    chain: Vec<ast::Node>,
}

struct OptionalChainRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
}

impl OptionalChainRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstTraversalState::store_for(self.source, self.factory(), node)
    }

    fn update_call_expression(
        &mut self,
        node: ast::Node,
        expression: ast::Node,
        arguments: ast::NodeList,
        flags: ast::NodeFlags,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_call_expression(node, expression, None, None, arguments, flags)
        } else {
            let source = self.source;
            self.factory_mut().update_call_expression_from_store(
                source, node, expression, None, None, arguments, flags,
            )
        }
    }

    fn update_parenthesized_expression(
        &mut self,
        node: ast::Node,
        expression: ast::Node,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_parenthesized_expression(node, expression)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_parenthesized_expression_from_store(source, node, expression)
        }
    }

    fn update_property_access_expression(
        &mut self,
        node: ast::Node,
        expression: ast::Node,
        name: Option<ast::Node>,
        flags: ast::NodeFlags,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_property_access_expression(node, expression, None, name, flags)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_property_access_expression_from_store(
                    source, node, expression, None, name, flags,
                )
        }
    }

    fn update_element_access_expression(
        &mut self,
        node: ast::Node,
        expression: ast::Node,
        argument_expression: Option<ast::Node>,
        flags: ast::NodeFlags,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_element_access_expression(
                node,
                expression,
                None,
                argument_expression,
                flags,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_element_access_expression_from_store(
                    source,
                    node,
                    expression,
                    None,
                    argument_expression,
                    flags,
                )
        }
    }

    fn restore_outer_expressions_in_current_store(
        &mut self,
        outer_expression: Option<ast::Node>,
        inner_expression: ast::Node,
        kinds: ast::OuterExpressionKinds,
    ) -> ast::Node {
        let Some(outer_expression) = outer_expression else {
            return inner_expression;
        };
        let is_outer_expression = {
            let store = self.factory().store();
            ast::is_outer_expression(store, outer_expression, kinds)
        };
        if !is_outer_expression
            || self
                .emit_context
                .factory
                .is_ignorable_paren(&outer_expression)
        {
            return inner_expression;
        }

        let (kind, expression, type_node, flags, type_arguments) = {
            let store = self.factory().store();
            (
                store.kind(outer_expression),
                store
                    .expression(outer_expression)
                    .expect("outer expression should have expression"),
                store.r#type(outer_expression),
                store.flags(outer_expression),
                store
                    .source_type_arguments(outer_expression)
                    .map(|type_arguments| {
                        (
                            type_arguments.loc(),
                            type_arguments.range(),
                            type_arguments.has_trailing_comma(),
                            type_arguments.iter().collect::<Vec<_>>(),
                        )
                    }),
            )
        };
        let expression = self.restore_outer_expressions_in_current_store(
            Some(expression),
            inner_expression,
            ast::OuterExpressionKinds::ALL,
        );
        match kind {
            ast::Kind::ParenthesizedExpression => self
                .factory_mut()
                .update_parenthesized_expression(outer_expression, expression),
            ast::Kind::TypeAssertionExpression => {
                self.factory_mut()
                    .update_type_assertion(outer_expression, type_node, expression)
            }
            ast::Kind::AsExpression => {
                self.factory_mut()
                    .update_as_expression(outer_expression, expression, type_node)
            }
            ast::Kind::SatisfiesExpression => self.factory_mut().update_satisfies_expression(
                outer_expression,
                expression,
                type_node,
            ),
            ast::Kind::NonNullExpression => {
                self.factory_mut()
                    .update_non_null_expression(outer_expression, expression, flags)
            }
            ast::Kind::ExpressionWithTypeArguments => {
                let type_arguments =
                    type_arguments.map(|(loc, range, has_trailing_comma, nodes)| {
                        self.factory_mut().new_node_list_with_trailing_comma(
                            loc,
                            range,
                            nodes,
                            has_trailing_comma,
                        )
                    });
                self.factory_mut().update_expression_with_type_arguments(
                    outer_expression,
                    expression,
                    type_arguments,
                )
            }
            ast::Kind::PartiallyEmittedExpression => self
                .factory_mut()
                .update_partially_emitted_expression(outer_expression, expression),
            _ => panic!("Unexpected outer expression kind: {:?}", kind),
        }
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let source = self.store_for(*node);
        if !self.contains_optional_chaining(*node) {
            return Some(*node);
        }
        match source.kind(*node) {
            ast::Kind::CallExpression => Some(self.visit_call_expression(*node, false)),
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression
                if source.flags(*node).contains(ast::NodeFlags::OPTIONAL_CHAIN) =>
            {
                Some(self.visit_optional_expression(*node, false, false))
            }
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                Some(self.generated_visit_each_child(node))
            }
            ast::Kind::DeleteExpression => Some(self.visit_delete_expression(*node)),
            _ => Some(self.generated_visit_each_child(node)),
        }
    }

    fn contains_optional_chaining(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        if store
            .subtree_facts(node)
            .intersects(ast::SubtreeFacts::CONTAINS_OPTIONAL_CHAINING)
            || store.flags(node).contains(ast::NodeFlags::OPTIONAL_CHAIN)
        {
            return true;
        }
        let mut found = false;
        let _ = store.for_each_present_child(node, |child| {
            if self.contains_optional_chaining(child) {
                found = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        found
    }

    fn visit_call_expression(&mut self, node: ast::Node, capture_this_arg: bool) -> ast::Node {
        let source = self.store_for(node);
        if source.flags(node).contains(ast::NodeFlags::OPTIONAL_CHAIN) {
            // If `node` is an optional chain, then it is the outermost chain of an optional expression.
            return self.visit_optional_expression(node, capture_this_arg, false);
        }

        if let Some(expression) = source.expression(node) {
            if ast::is_parenthesized_expression(source, expression) {
                let unwrapped = ast::skip_parentheses(source, expression);
                if source
                    .flags(unwrapped)
                    .contains(ast::NodeFlags::OPTIONAL_CHAIN)
                {
                    // capture thisArg for calls of parenthesized optional chains like `(foo?.bar)()`
                    let expression = self.visit_parenthesized_expression(expression, true, false);
                    let args = self.visit_arguments_vec(node);
                    if ast::is_synthetic_reference_expression(
                        self.store_for(expression),
                        expression,
                    ) {
                        let store = self.store_for(expression);
                        let target = store
                            .expression(expression)
                            .expect("synthetic reference should have expression");
                        let this_arg = store.this_arg(expression);
                        let target = self.preserve_node(target);
                        let this_arg = this_arg.map(|this_arg| self.preserve_node(this_arg));
                        let res = self.emit_context.factory.new_function_call_call(
                            &target,
                            this_arg.as_ref(),
                            &args,
                        );
                        let loc = self.store_for(node).loc(node);
                        self.factory_mut().place_emit_synthetic_node(res, loc);
                        self.emit_context.set_original(&res, &node);
                        return res;
                    }
                    let arguments = self.new_node_list_like_arguments(node, args);
                    let flags = self.store_for(node).flags(node);
                    return self.update_call_expression(node, expression, arguments, flags);
                }
            }
        }

        self.generated_visit_each_child(&node)
    }

    fn visit_parenthesized_expression(
        &mut self,
        node: ast::Node,
        capture_this_arg: bool,
        is_delete: bool,
    ) -> ast::Node {
        let source = self.store_for(node);
        let expression = source
            .expression(node)
            .expect("parenthesized expression should have expression");
        let expr = self.visit_non_optional_expression(expression, capture_this_arg, is_delete);
        if ast::is_synthetic_reference_expression(self.store_for(expr), expr) {
            // `(a.b)` -> { expression `((_a = a).b)`, thisArg: `_a` }
            // `(a[b])` -> { expression `((_a = a)[b])`, thisArg: `_a` }
            let store = self.store_for(expr);
            let synthetic_expression = store
                .expression(expr)
                .expect("synthetic reference should have expression");
            let this_arg = store
                .this_arg(expr)
                .expect("synthetic reference should have this argument");
            let synthetic_expression = self.preserve_node(synthetic_expression);
            let this_arg = self.preserve_node(this_arg);
            let parenthesized = self.update_parenthesized_expression(node, synthetic_expression);
            let res = self
                .factory_mut()
                .new_synthetic_reference_expression(parenthesized, this_arg);
            self.emit_context.set_original(&res, &node);
            return res;
        }
        self.update_parenthesized_expression(node, expr)
    }

    fn visit_property_or_element_access_expression(
        &mut self,
        node: ast::Node,
        capture_this_arg: bool,
        is_delete: bool,
    ) -> ast::Node {
        let source = self.store_for(node);
        if source.flags(node).contains(ast::NodeFlags::OPTIONAL_CHAIN) {
            // If `node` is an optional chain, then it is the outermost chain of an optional expression.
            return self.visit_optional_expression(node, capture_this_arg, is_delete);
        }

        let expression = source
            .expression(node)
            .expect("access expression should have expression");
        let mut expression = self
            .visit_node(Some(expression))
            .expect("access expression should keep expression");
        let mut this_arg = None;
        if capture_this_arg {
            if !self.is_simple_copiable(expression) {
                let temp = self.emit_context.factory.new_temp_variable();
                self.emit_context.add_variable_declaration(temp);
                expression = self
                    .emit_context
                    .factory
                    .new_assignment_expression(temp, expression);
                this_arg = Some(temp);
            } else {
                this_arg = Some(expression);
            }
        }

        let (node_kind, name, argument_expression, flags) = {
            let source = self.store_for(node);
            (
                source.kind(node),
                source.name(node),
                source.argument_expression(node),
                source.flags(node),
            )
        };
        expression = match node_kind {
            ast::Kind::PropertyAccessExpression => {
                let name = name.and_then(|name| self.visit_node(Some(name)));
                self.update_property_access_expression(node, expression, name, flags)
            }
            ast::Kind::ElementAccessExpression => {
                let argument_expression =
                    argument_expression.and_then(|argument| self.visit_node(Some(argument)));
                self.update_element_access_expression(node, expression, argument_expression, flags)
            }
            _ => unreachable!("expected property or element access expression"),
        };

        if let Some(this_arg) = this_arg {
            let res = self
                .factory_mut()
                .new_synthetic_reference_expression(expression, this_arg);
            self.emit_context.set_original(&res, &node);
            return res;
        }
        expression
    }

    fn visit_delete_expression(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let expression = source
            .expression(node)
            .expect("delete expression should have expression");
        let unwrapped = ast::skip_parentheses(source, expression);
        if source
            .flags(unwrapped)
            .contains(ast::NodeFlags::OPTIONAL_CHAIN)
        {
            return self.visit_non_optional_expression(expression, false, true);
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_non_optional_expression(
        &mut self,
        node: ast::Node,
        capture_this_arg: bool,
        is_delete: bool,
    ) -> ast::Node {
        match self.store_for(node).kind(node) {
            ast::Kind::ParenthesizedExpression => {
                self.visit_parenthesized_expression(node, capture_this_arg, is_delete)
            }
            ast::Kind::ElementAccessExpression | ast::Kind::PropertyAccessExpression => {
                self.visit_property_or_element_access_expression(node, capture_this_arg, is_delete)
            }
            ast::Kind::CallExpression => self.visit_call_expression(node, capture_this_arg),
            _ => self
                .visit_node(Some(node))
                .expect("expression visitor should keep expression"),
        }
    }

    fn flatten_chain(&self, chain: ast::Node) -> FlattenResult {
        let mut current = chain;
        let mut links = vec![current];
        while !ast::is_tagged_template_expression(self.store_for(current), current)
            && self
                .store_for(current)
                .question_dot_token(current)
                .is_none()
        {
            let source = self.store_for(current);
            current = ast::skip_partially_emitted_expressions(
                source,
                source
                    .expression(current)
                    .expect("optional chain segment should have expression"),
            );
            links.insert(0, current);
        }
        FlattenResult {
            expression: self
                .store_for(current)
                .expression(current)
                .expect("optional chain head should have expression"),
            chain: links,
        }
    }

    fn visit_optional_expression(
        &mut self,
        node: ast::Node,
        capture_this_arg: bool,
        is_delete: bool,
    ) -> ast::Node {
        let FlattenResult { expression, chain } = self.flatten_chain(node);
        let first_chain_store = self.store_for(chain[0]);
        let first_is_call_chain = first_chain_store.kind(chain[0]) == ast::Kind::CallExpression
            && first_chain_store
                .flags(chain[0])
                .contains(ast::NodeFlags::OPTIONAL_CHAIN);
        let expression_store = self.store_for(expression);
        let left = self.visit_non_optional_expression(
            ast::skip_partially_emitted_expressions(expression_store, expression),
            first_is_call_chain,
            false,
        );
        let mut left_this_arg = None;
        let mut captured_left = left;
        if ast::is_synthetic_reference_expression(self.store_for(left), left) {
            let store = self.store_for(left);
            left_this_arg = store.this_arg(left);
            captured_left = store
                .expression(left)
                .expect("synthetic reference should have expression");
        }
        let captured_left = self.preserve_node(captured_left);
        let mut left_expression = if expression.store_id() == self.factory().store().store_id() {
            self.restore_outer_expressions_in_current_store(
                Some(expression),
                captured_left,
                ast::OuterExpressionKinds::PARTIALLY_EMITTED_EXPRESSIONS,
            )
        } else {
            self.emit_context.factory.restore_outer_expressions(
                self.source,
                Some(&expression),
                &captured_left,
                ast::OuterExpressionKinds::PARTIALLY_EMITTED_EXPRESSIONS,
            )
        };
        let mut captured_left = captured_left;
        if !self.is_simple_copiable(captured_left) {
            let temp = self.emit_context.factory.new_temp_variable();
            self.emit_context.add_variable_declaration(temp);
            left_expression = self
                .emit_context
                .factory
                .new_assignment_expression(temp, left_expression);
            captured_left = temp;
        }

        let mut right_expression = captured_left;
        let mut this_arg = None;

        for (i, segment) in chain.iter().copied().enumerate() {
            let (segment_kind, argument_expression, name) = {
                let segment_store = self.store_for(segment);
                (
                    segment_store.kind(segment),
                    segment_store.argument_expression(segment),
                    segment_store.name(segment),
                )
            };
            match segment_kind {
                ast::Kind::ElementAccessExpression | ast::Kind::PropertyAccessExpression => {
                    if i + 1 == chain.len() && capture_this_arg {
                        if !self.is_simple_copiable(right_expression) {
                            let temp = self.emit_context.factory.new_temp_variable();
                            self.emit_context.add_variable_declaration(temp);
                            right_expression = self
                                .emit_context
                                .factory
                                .new_assignment_expression(temp, right_expression);
                            this_arg = Some(temp);
                        } else {
                            this_arg = Some(right_expression);
                        }
                    }
                    right_expression = if segment_kind == ast::Kind::ElementAccessExpression {
                        let argument = argument_expression
                            .and_then(|argument| self.visit_node(Some(argument)));
                        self.factory_mut().new_element_access_expression(
                            right_expression,
                            None,
                            argument,
                            ast::NodeFlags::NONE,
                        )
                    } else {
                        let name = name.and_then(|name| self.visit_node(Some(name)));
                        self.factory_mut().new_property_access_expression(
                            right_expression,
                            None,
                            name,
                            ast::NodeFlags::NONE,
                        )
                    };
                }
                ast::Kind::CallExpression => {
                    if i == 0 {
                        if let Some(left_this_arg) = left_this_arg {
                            let left_this_arg = if self
                                .emit_context
                                .has_auto_generate_info(Some(&left_this_arg))
                            {
                                self.preserve_node(left_this_arg)
                            } else {
                                let imported = self.preserve_node(left_this_arg);
                                let cloned = self.factory_mut().clone_node(imported);
                                self.emit_context
                                    .add_emit_flags(&cloned, printer::EF_NO_COMMENTS);
                                cloned
                            };
                            let call_this_arg = if self.store_for(left_this_arg).kind(left_this_arg)
                                == ast::Kind::SuperKeyword
                            {
                                self.emit_context.factory.new_this_expression()
                            } else {
                                left_this_arg
                            };
                            let args = self.visit_arguments_vec(segment);
                            right_expression = self.emit_context.factory.new_function_call_call(
                                &right_expression,
                                Some(&call_this_arg),
                                &args,
                            );
                            self.emit_context.set_original(&right_expression, &segment);
                            continue;
                        }
                    }
                    let visited_args = self.visit_arguments_vec(segment);
                    let args = self.new_node_list_like_arguments(segment, visited_args);
                    right_expression = self.factory_mut().new_call_expression(
                        right_expression,
                        None,
                        None,
                        args,
                        ast::NodeFlags::NONE,
                    );
                }
                _ => {}
            }
            self.emit_context.set_original(&right_expression, &segment);
        }

        let condition = self.create_not_null_condition(left_expression, captured_left, true);
        let question_token = self.factory_mut().new_token(ast::Kind::QuestionToken);
        let colon_token = self.factory_mut().new_token(ast::Kind::ColonToken);
        let target = if is_delete {
            let when_true = self.emit_context.factory.new_true_expression();
            let when_false = self.factory_mut().new_delete_expression(right_expression);
            self.factory_mut().new_conditional_expression(
                condition,
                question_token,
                when_true,
                colon_token,
                when_false,
            )
        } else {
            let when_true = self.emit_context.factory.new_void_zero_expression();
            self.factory_mut().new_conditional_expression(
                condition,
                question_token,
                when_true,
                colon_token,
                right_expression,
            )
        };
        let loc = self.store_for(node).loc(node);
        self.factory_mut().place_emit_synthetic_node(target, loc);
        let target = if let Some(this_arg) = this_arg {
            self.factory_mut()
                .new_synthetic_reference_expression(target, this_arg)
        } else {
            target
        };
        self.emit_context.set_original(&target, &node);
        target
    }

    fn create_not_null_condition(
        &mut self,
        left: ast::Node,
        right: ast::Node,
        invert: bool,
    ) -> ast::Node {
        let (equality, combine) = if invert {
            (ast::Kind::EqualsEqualsEqualsToken, ast::Kind::BarBarToken)
        } else {
            (
                ast::Kind::ExclamationEqualsEqualsToken,
                ast::Kind::AmpersandAmpersandToken,
            )
        };
        let null = self
            .factory_mut()
            .new_keyword_expression(ast::Kind::NullKeyword);
        let equality_token = self.factory_mut().new_token(equality);
        let left_condition =
            self.factory_mut()
                .new_binary_expression(None, left, None, equality_token, null);
        let void_zero = self.emit_context.factory.new_void_zero_expression();
        let equality_token = self.factory_mut().new_token(equality);
        let right_condition =
            self.factory_mut()
                .new_binary_expression(None, right, None, equality_token, void_zero);
        let combine_token = self.factory_mut().new_token(combine);
        self.factory_mut().new_binary_expression(
            None,
            left_condition,
            None,
            combine_token,
            right_condition,
        )
    }

    fn is_simple_copiable(&self, expression: ast::Node) -> bool {
        crate::utilities::is_simple_copiable_expression(self.store_for(expression), &expression)
    }

    fn visit_arguments_vec(&mut self, call: ast::Node) -> Vec<ast::Node> {
        let arguments = {
            let Some(arguments) = self.store_for(call).source_arguments(call) else {
                return Vec::new();
            };
            arguments.iter().collect::<Vec<_>>()
        };
        arguments
            .into_iter()
            .filter_map(|argument| self.visit_node(Some(argument)))
            .collect()
    }

    fn new_node_list_like_arguments(
        &mut self,
        call: ast::Node,
        arguments: Vec<ast::Node>,
    ) -> ast::NodeList {
        let Some((loc, range)) = ({
            self.store_for(call)
                .source_arguments(call)
                .map(|source_arguments| (source_arguments.loc(), source_arguments.range()))
        }) else {
            return self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                arguments,
            );
        };
        self.factory_mut().new_node_list(loc, range, arguments)
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

impl<'source> ast::AstVisitEachChildRuntime<'source> for OptionalChainRuntime<'_, 'source> {
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for OptionalChainRuntime<'_, 'source> {}
