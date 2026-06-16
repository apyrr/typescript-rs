use std::collections::HashSet;

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

use super::utilities::{self, SuperAccessState};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AsyncContextFlags {
    pub non_top_level: bool,
    pub has_lexical_this: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncAction {
    KeepFallback,
    VisitChildren,
    ElideAsyncKeyword,
    VisitSourceFile,
    RewriteAwaitToYield,
    PreserveTopLevelAwait,
    VisitAsyncFunctionLike,
    VisitNonAsyncFunctionLike,
    VisitArrowFunction,
    VisitClassBoundary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncBodyAction {
    VisitChildren,
    RewriteVariableStatementWithCollidingNames,
    RewriteForStatementWithCollidingNames,
    RewriteForInOrOfWithCollidingNames,
    RewriteCatchClauseShadowingParameters,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AsyncFacts {
    pub subtree_contains_any_await: bool,
    pub subtree_contains_await: bool,
    pub is_declaration_file: bool,
    pub in_top_level_context: bool,
    pub function_is_async: bool,
    pub function_has_simple_parameter_list: bool,
    pub lexical_arguments_binding_exists: bool,
    pub identifier_is_arguments_reference: bool,
    pub variable_declaration_list_collides_with_parameter: bool,
    pub catch_clause_shadows_parameter: bool,
    pub has_super_element_access: bool,
    pub has_super_property_assignment: bool,
    pub captured_super_property_count: usize,
}

pub fn async_action_for_kind(kind: ast::Kind, facts: AsyncFacts) -> AsyncAction {
    if !(facts.subtree_contains_any_await || facts.subtree_contains_await) {
        return AsyncAction::KeepFallback;
    }

    match kind {
        ast::Kind::AsyncKeyword => AsyncAction::ElideAsyncKeyword,
        ast::Kind::SourceFile => AsyncAction::VisitSourceFile,
        ast::Kind::AwaitExpression if facts.in_top_level_context => {
            AsyncAction::PreserveTopLevelAwait
        }
        ast::Kind::AwaitExpression => AsyncAction::RewriteAwaitToYield,
        ast::Kind::MethodDeclaration
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
            if facts.function_is_async =>
        {
            AsyncAction::VisitAsyncFunctionLike
        }
        ast::Kind::MethodDeclaration
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression => AsyncAction::VisitNonAsyncFunctionLike,
        ast::Kind::ArrowFunction => AsyncAction::VisitArrowFunction,
        ast::Kind::GetAccessor | ast::Kind::SetAccessor | ast::Kind::Constructor => {
            AsyncAction::VisitNonAsyncFunctionLike
        }
        ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => AsyncAction::VisitClassBoundary,
        _ => AsyncAction::VisitChildren,
    }
}

pub fn async_body_action_for_kind(kind: ast::Kind, facts: AsyncFacts) -> AsyncBodyAction {
    match kind {
        ast::Kind::VariableStatement if facts.variable_declaration_list_collides_with_parameter => {
            AsyncBodyAction::RewriteVariableStatementWithCollidingNames
        }
        ast::Kind::ForStatement if facts.variable_declaration_list_collides_with_parameter => {
            AsyncBodyAction::RewriteForStatementWithCollidingNames
        }
        ast::Kind::ForInStatement | ast::Kind::ForOfStatement
            if facts.variable_declaration_list_collides_with_parameter =>
        {
            AsyncBodyAction::RewriteForInOrOfWithCollidingNames
        }
        ast::Kind::CatchClause if facts.catch_clause_shadows_parameter => {
            AsyncBodyAction::RewriteCatchClauseShadowingParameters
        }
        _ => AsyncBodyAction::VisitChildren,
    }
}

pub fn async_function_needs_parameter_wrapper(function_has_simple_parameter_list: bool) -> bool {
    !function_has_simple_parameter_list
}

pub fn should_capture_lexical_arguments(
    lexical_arguments_binding_exists: bool,
    identifier_is_arguments_reference: bool,
) -> bool {
    lexical_arguments_binding_exists && identifier_is_arguments_reference
}

pub fn should_emit_capture_arguments_statement(lexical_arguments_was_used: bool) -> bool {
    lexical_arguments_was_used
}

pub fn async_super_helper_kind(
    has_super_element_access: bool,
    has_super_property_assignment: bool,
) -> super::forawait::SuperHelperKind {
    if !has_super_element_access {
        super::forawait::SuperHelperKind::None
    } else if has_super_property_assignment {
        super::forawait::SuperHelperKind::AdvancedAsyncSuper
    } else {
        super::forawait::SuperHelperKind::AsyncSuper
    }
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
) -> ast::Node {
    if source_file.is_declaration_file() {
        return root;
    }

    let mut runtime = AsyncRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        context_flags: AsyncContextFlags::default(),
        enclosing_function_parameter_names: None,
        lexical_arguments: LexicalArguments::default(),
        super_access_state: None,
        super_binding: None,
        super_index_binding: None,
        substitute_super_accesses: false,
        visiting_async_body: false,
        parent_node: None,
        current_node: None,
    };
    runtime.context_flags.non_top_level = false;
    runtime.context_flags.has_lexical_this = false;
    let root = runtime.visit_node(Some(root)).unwrap_or(root);
    runtime.emit_context.add_requested_emit_helpers(&root);
    root
}

#[derive(Clone, Copy, Debug, Default)]
struct LexicalArguments {
    binding: Option<ast::Node>,
    used: bool,
}

struct AsyncRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    context_flags: AsyncContextFlags,
    enclosing_function_parameter_names: Option<HashSet<String>>,
    lexical_arguments: LexicalArguments,
    super_access_state: Option<SuperAccessState>,
    super_binding: Option<ast::Node>,
    super_index_binding: Option<ast::Node>,
    substitute_super_accesses: bool,
    visiting_async_body: bool,
    parent_node: Option<ast::Node>,
    current_node: Option<ast::Node>,
}

impl AsyncRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        self.emit_context.store_for_node(node)
    }

    fn is_factory_node(&self, node: ast::Node) -> bool {
        node.store_id() == self.factory().store().store_id()
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

    fn source_parameters_input(&self, node: ast::Node) -> Option<ast::SourceNodeListInput> {
        self.store_for(node)
            .source_parameters(node)
            .map(ast::SourceNodeListInput::from_source)
    }

    fn source_modifiers_input(&self, node: ast::Node) -> Option<ast::SourceModifierListInput> {
        self.store_for(node)
            .source_modifiers(node)
            .map(ast::SourceModifierListInput::from_source)
    }

    fn function_flags(&self, node: ast::Node) -> ast::FunctionFlags {
        ast::get_function_flags(self.store_for(node), Some(node))
    }

    fn update_arrow_function(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        parameters: ast::NodeList,
        equals: Option<ast::Node>,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory_mut().update_arrow_function(
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
            let source = self.source;
            self.factory_mut().update_arrow_function_from_store(
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
    }

    fn update_constructor_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory_mut().update_constructor_declaration(
                node,
                modifiers,
                None::<ast::NodeList>,
                parameters,
                None::<ast::Node>,
                None::<ast::Node>,
                body,
            )
        } else {
            let source = self.source;
            self.factory_mut()
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
    }

    fn update_method_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        asterisk: Option<ast::Node>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory_mut().update_method_declaration(
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
            let source = self.source;
            self.factory_mut().update_method_declaration_from_store(
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
    }

    fn update_get_accessor_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory_mut().update_get_accessor_declaration(
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
            let source = self.source;
            self.factory_mut()
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
    }

    fn update_set_accessor_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory_mut().update_set_accessor_declaration(
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
            let source = self.source;
            self.factory_mut()
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
    }

    fn update_function_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        asterisk: Option<ast::Node>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory_mut().update_function_declaration(
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
            let source = self.source;
            self.factory_mut().update_function_declaration_from_store(
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
    }

    fn update_function_expression(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        asterisk: Option<ast::Node>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if self.is_factory_node(node) {
            self.factory_mut().update_function_expression(
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
            let source = self.source;
            self.factory_mut().update_function_expression_from_store(
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

    fn with_async_body_visitor<R>(&mut self, cb: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.visiting_async_body;
        self.visiting_async_body = true;
        let result = cb(self);
        self.visiting_async_body = saved;
        result
    }

    fn with_context<R>(&mut self, flags: AsyncContextFlags, cb: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.context_flags;
        self.context_flags.non_top_level |= flags.non_top_level;
        self.context_flags.has_lexical_this |= flags.has_lexical_this;
        let result = cb(self);
        self.context_flags = saved;
        result
    }

    fn in_top_level_context(&self) -> bool {
        !self.context_flags.non_top_level
    }

    fn in_has_lexical_this_context(&self) -> bool {
        self.context_flags.has_lexical_this
    }

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        let kind = self.store_for(*node).kind(*node);
        self.descend_into(*node, |this| {
            if this.substitute_super_accesses
                && let Some(substituted) = this.try_substitute_super_access(*node)
            {
                return Some(substituted);
            }
            if !this.contains_async_transform(*node) {
                return this.visit_fallback(*node);
            }
            match kind {
                ast::Kind::AsyncKeyword => None,
                ast::Kind::SourceFile => Some(this.generated_visit_each_child(node)),
                ast::Kind::AwaitExpression => Some(this.visit_await_expression(*node)),
                ast::Kind::MethodDeclaration => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: true,
                    },
                    |this| Some(this.visit_method_declaration(*node)),
                ),
                ast::Kind::FunctionDeclaration => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: true,
                    },
                    |this| Some(this.visit_function_declaration(*node)),
                ),
                ast::Kind::FunctionExpression => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: true,
                    },
                    |this| Some(this.visit_function_expression(*node)),
                ),
                ast::Kind::ArrowFunction => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: false,
                    },
                    |this| Some(this.visit_arrow_function(*node)),
                ),
                ast::Kind::GetAccessor => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: true,
                    },
                    |this| Some(this.visit_get_accessor_declaration(*node)),
                ),
                ast::Kind::SetAccessor => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: true,
                    },
                    |this| Some(this.visit_set_accessor_declaration(*node)),
                ),
                ast::Kind::Constructor => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: true,
                    },
                    |this| Some(this.visit_constructor_declaration(*node)),
                ),
                ast::Kind::ClassDeclaration | ast::Kind::ClassExpression => this.with_context(
                    AsyncContextFlags {
                        non_top_level: true,
                        has_lexical_this: true,
                    },
                    |this| Some(this.generated_visit_each_child(node)),
                ),
                _ => Some(this.generated_visit_each_child(node)),
            }
        })
    }

    fn visit_fallback(&mut self, node: ast::Node) -> Option<ast::Node> {
        let store = self.store_for(node);
        match store.kind(node) {
            ast::Kind::FunctionExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::Constructor => return Some(node),
            ast::Kind::Identifier
                if self.lexical_arguments.binding.is_some()
                    && store.text(node) == "arguments"
                    && !self.is_name_of_property_access_or_assignment(node) =>
            {
                self.lexical_arguments.used = true;
                return self.lexical_arguments.binding;
            }
            _ => {}
        }
        Some(self.generated_visit_each_child(&node))
    }

    fn contains_async_transform(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        store
            .subtree_facts(node)
            .intersects(ast::SubtreeFacts::CONTAINS_ANY_AWAIT | ast::SubtreeFacts::CONTAINS_AWAIT)
            || matches!(
                store.kind(node),
                ast::Kind::AwaitExpression | ast::Kind::AsyncKeyword
            )
            || ast::get_function_flags(store, Some(node)) & ast::FUNCTION_FLAGS_ASYNC != 0
            || {
                let mut found = false;
                let _ = store.for_each_present_child(node, |child| {
                    if self.contains_async_transform(child) {
                        found = true;
                        std::ops::ControlFlow::Break(())
                    } else {
                        std::ops::ControlFlow::Continue(())
                    }
                });
                found
            }
    }

    fn try_substitute_super_access(&mut self, node: ast::Node) -> Option<ast::Node> {
        self.super_access_state.as_ref()?;
        match self.store_for(node).kind(node) {
            ast::Kind::CallExpression => {
                let expression = self.store_for(node).expression(node)?;
                if ast::is_property_access_expression(self.store_for(expression), expression)
                    && self.expression_is_super(expression)
                {
                    return Some(
                        self.substitute_call_expression_with_super_access(node, expression),
                    );
                }
                if ast::is_element_access_expression(self.store_for(expression), expression)
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
                let name = self
                    .store_for(node)
                    .name(node)
                    .map(|name| self.preserve_node(name));
                Some(self.factory_mut().new_property_access_expression(
                    super_binding,
                    None::<ast::Node>,
                    name,
                    ast::NodeFlags::NONE,
                ))
            }
            ast::Kind::ElementAccessExpression if self.expression_is_super(node) => {
                let argument = self
                    .store_for(node)
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
        let expression_store = self.store_for(expression);
        let target = if ast::is_property_access_expression(expression_store, expression) {
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
        } else if ast::is_element_access_expression(expression_store, expression) {
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
        let source_arguments = {
            let source = self.store_for(call);
            source
                .source_arguments(call)
                .map(|arguments| arguments.iter().collect::<Vec<_>>())
        };
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
        let expression_is_super = self.expression_is_super(node);
        let source = self.store_for(node);
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
            ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => source
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
        let source = self.store_for(node);
        source.expression(node).is_some_and(|expression| {
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

    fn visit_await_expression(&mut self, node: ast::Node) -> ast::Node {
        if self.in_top_level_context() {
            return self.generated_visit_each_child(&node);
        }
        let (expression, loc) = {
            let source = self.store_for(node);
            (source.expression(node), source.loc(node))
        };
        let expression = expression.and_then(|node| self.visit_node(Some(node)));
        let yield_expr = self
            .factory_mut()
            .new_yield_expression(None::<ast::Node>, expression);
        self.factory_mut()
            .place_emit_synthetic_node(yield_expr, loc);
        self.emit_context.set_original(&yield_expr, &node);
        yield_expr
    }

    fn visit_constructor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let saved_lexical_arguments = self.lexical_arguments;
        self.lexical_arguments = LexicalArguments::default();
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("constructor parameters should exist");
        let body = self.transform_method_body(node);
        self.lexical_arguments = saved_lexical_arguments;
        self.update_constructor_declaration(node, modifiers, parameters, body)
    }

    fn visit_method_declaration(&mut self, node: ast::Node) -> ast::Node {
        let saved_lexical_arguments = self.lexical_arguments;
        self.lexical_arguments = LexicalArguments::default();
        let (parameters, body) = if self.function_flags(node) & ast::FUNCTION_FLAGS_ASYNC != 0 {
            let (parameters, outer_parameter_names) =
                self.transform_async_function_parameter_list(node);
            let body = self.transform_async_function_body(node, &outer_parameter_names);
            (parameters, Some(body))
        } else {
            (
                self.visit_parameters_input(self.source_parameters_input(node))
                    .expect("method parameters should exist"),
                self.transform_method_body(node),
            )
        };
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let (asterisk, name) = {
            let source = self.store_for(node);
            (source.asterisk_token(node), source.name(node))
        };
        let asterisk = asterisk.map(|node| self.preserve_node(node));
        let name = name.map(|node| self.preserve_node(node));
        self.lexical_arguments = saved_lexical_arguments;
        self.update_method_declaration(node, modifiers, asterisk, name, parameters, body)
    }

    fn visit_get_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let saved_lexical_arguments = self.lexical_arguments;
        self.lexical_arguments = LexicalArguments::default();
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let name = self.store_for(node).name(node);
        let name = name.map(|node| self.preserve_node(node));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("accessor parameters should exist");
        let body = self.transform_method_body(node);
        self.lexical_arguments = saved_lexical_arguments;
        self.update_get_accessor_declaration(node, modifiers, name, parameters, body)
    }

    fn visit_set_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let saved_lexical_arguments = self.lexical_arguments;
        self.lexical_arguments = LexicalArguments::default();
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let name = self.store_for(node).name(node);
        let name = name.map(|node| self.preserve_node(node));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("accessor parameters should exist");
        let body = self.transform_method_body(node);
        self.lexical_arguments = saved_lexical_arguments;
        self.update_set_accessor_declaration(node, modifiers, name, parameters, body)
    }

    fn visit_function_declaration(&mut self, node: ast::Node) -> ast::Node {
        let saved_lexical_arguments = self.lexical_arguments;
        self.lexical_arguments = LexicalArguments::default();
        let (parameters, body) = if self.function_flags(node) & ast::FUNCTION_FLAGS_ASYNC != 0 {
            let (parameters, outer_parameter_names) =
                self.transform_async_function_parameter_list(node);
            let body = self.transform_async_function_body(node, &outer_parameter_names);
            (parameters, Some(body))
        } else {
            let body = self.store_for(node).body(node);
            (
                self.visit_parameters_input(self.source_parameters_input(node))
                    .expect("function parameters should exist"),
                self.visit_function_body(body),
            )
        };
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let (asterisk, name) = {
            let source = self.store_for(node);
            (source.asterisk_token(node), source.name(node))
        };
        let asterisk = asterisk.map(|node| self.preserve_node(node));
        let name = name.and_then(|node| self.visit_node(Some(node)));
        self.lexical_arguments = saved_lexical_arguments;
        self.update_function_declaration(node, modifiers, asterisk, name, parameters, body)
    }

    fn visit_function_expression(&mut self, node: ast::Node) -> ast::Node {
        let saved_lexical_arguments = self.lexical_arguments;
        self.lexical_arguments = LexicalArguments::default();
        let (parameters, body) = if self.function_flags(node) & ast::FUNCTION_FLAGS_ASYNC != 0 {
            let (parameters, outer_parameter_names) =
                self.transform_async_function_parameter_list(node);
            let body = self.transform_async_function_body(node, &outer_parameter_names);
            (parameters, Some(body))
        } else {
            let body = self.store_for(node).body(node);
            (
                self.visit_parameters_input(self.source_parameters_input(node))
                    .expect("function parameters should exist"),
                self.visit_function_body(body),
            )
        };
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let (asterisk, name) = {
            let source = self.store_for(node);
            (source.asterisk_token(node), source.name(node))
        };
        let asterisk = asterisk.map(|node| self.preserve_node(node));
        let name = name.and_then(|node| self.visit_node(Some(node)));
        self.lexical_arguments = saved_lexical_arguments;
        self.update_function_expression(node, modifiers, asterisk, name, parameters, body)
    }

    fn visit_arrow_function(&mut self, node: ast::Node) -> ast::Node {
        // `arguments` in class static blocks is always an error, but we preserve Strada's emit
        // behavior for baseline compatibility. In Strada, checker-based `isArgumentsLocalBinding`
        // returns false for `arguments` in static blocks (since the binding doesn't exist due to
        // the error), so the async transform leaves them untouched.
        let saved_lexical_arguments = self.lexical_arguments;
        let suppress_lexical_arguments =
            self.emit_context.emit_flags(&node) & printer::EF_NO_LEXICAL_ARGUMENTS != 0;
        if suppress_lexical_arguments {
            self.lexical_arguments = LexicalArguments::default();
        }
        let function_flags = {
            let source = self.store_for(node);
            ast::get_function_flags(source, Some(node))
        };
        let (parameters, body) = if function_flags & ast::FUNCTION_FLAGS_ASYNC != 0 {
            let (parameters, outer_parameter_names) =
                self.transform_async_function_parameter_list(node);
            let body = self.transform_async_function_body(node, &outer_parameter_names);
            (parameters, Some(body))
        } else {
            let (parameters, body) = {
                let source = self.store_for(node);
                (
                    source
                        .source_parameters(node)
                        .map(ast::SourceNodeListInput::from_source),
                    source.body(node),
                )
            };
            (
                self.visit_parameters_input(parameters)
                    .expect("arrow parameters should exist"),
                self.visit_function_body(body),
            )
        };
        let (modifiers, equals) = {
            let source = self.store_for(node);
            (
                source
                    .source_modifiers(node)
                    .map(ast::SourceModifierListInput::from_source),
                source.equals_greater_than_token(node),
            )
        };
        let modifiers = self.visit_modifiers_input(modifiers);
        let equals = equals.map(|node| self.preserve_node(node));
        let updated = self.update_arrow_function(node, modifiers, parameters, equals, body);
        if suppress_lexical_arguments {
            self.lexical_arguments = saved_lexical_arguments;
        }
        updated
    }

    fn transform_method_body(&mut self, node: ast::Node) -> Option<ast::Node> {
        let body = self.store_for(node).body(node);
        let saved_super_state = self.super_access_state.clone();
        let saved_super_binding = self.super_binding;
        let saved_super_index_binding = self.super_index_binding;
        self.super_access_state = Some(self.new_super_state_for_body(body));
        self.super_binding = Some(self.new_super_binding("_super"));
        self.super_index_binding = Some(self.new_super_binding("_superIndex"));

        self.emit_context.start_variable_environment();
        let mut updated = self.visit_function_body(body);

        let original = self.get_original_if_function_like(node);
        let original_store = self.emit_context.store_for_node(original);
        let emit_super_helpers = self.super_access_state.as_ref().is_some_and(|state| {
            !state.captured_super_properties.is_empty() || state.has_super_element_access
        }) && ast::get_function_flags(original_store, Some(original))
            & ast::FUNCTION_FLAGS_ASYNC_GENERATOR
            != ast::FUNCTION_FLAGS_ASYNC_GENERATOR;

        if emit_super_helpers
            && self
                .super_access_state
                .as_ref()
                .is_some_and(|state| !state.captured_super_properties.is_empty())
        {
            let statement = self.create_super_access_variable_statement();
            self.emit_context.add_initialization_statement(statement);
        }

        if let Some(block) = updated {
            let statements = self
                .store_for(block)
                .source_statements(block)
                .expect("method body should be a block")
                .iter()
                .collect::<Vec<_>>();
            let statements = self
                .emit_context
                .end_and_merge_variable_environment(self.source, &statements);
            let statements = self.new_node_list_like_block(block, statements);
            let multi_line = self.store_for(block).multi_line(block).unwrap_or(true);
            updated = Some(
                self.factory_mut()
                    .update_block(block, statements, multi_line),
            );
        } else {
            let _ = self.emit_context.end_variable_environment();
        }

        if emit_super_helpers
            && self
                .super_access_state
                .as_ref()
                .is_some_and(|state| state.has_super_element_access)
            && let Some(block) = updated
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
        updated
    }

    fn get_original_if_function_like(&self, node: ast::Node) -> ast::Node {
        let original = self.emit_context.most_original(&node);
        let store = self.emit_context.store_for_node(original);
        if ast::is_function_like_declaration(store, Some(original)) {
            original
        } else {
            node
        }
    }

    fn transform_async_function_parameter_list(
        &mut self,
        node: ast::Node,
    ) -> (ast::NodeList, Vec<ast::Node>) {
        if self.is_simple_parameter_list(node) {
            let parameters = {
                let source = self.store_for(node);
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source)
            };
            let parameters = self
                .visit_parameters_input(parameters)
                .expect("async function parameters should exist");
            return (parameters, Vec::new());
        }

        let (source_list, is_arrow) = {
            let source = self.store_for(node);
            (
                ast::SourceNodeListInput::from_source(
                    source
                        .source_parameters(node)
                        .expect("async function parameters should exist"),
                ),
                source.kind(node) == ast::Kind::ArrowFunction,
            )
        };
        let mut new_parameters = Vec::new();
        let mut new_parameter_names = Vec::new();
        for parameter in source_list.iter() {
            let parameter_store = self.store_for(parameter);
            if parameter_store.initializer(parameter).is_some()
                || parameter_store.dot_dot_dot_token(parameter).is_some()
            {
                if is_arrow {
                    let dotdotdot = self.factory_mut().new_token(ast::Kind::DotDotDotToken);
                    let name = self.emit_context.factory.new_unique_name_ex(
                        "args",
                        printer::AutoGenerateOptions {
                            flags: printer::GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
                            ..Default::default()
                        },
                    );
                    let rest = self.factory_mut().new_parameter_declaration(
                        None::<ast::ModifierList>,
                        dotdotdot,
                        name,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        None::<ast::Node>,
                    );
                    new_parameter_names.push(name);
                    new_parameters.push(rest);
                }
                break;
            }
            let source_name = parameter_store
                .name(parameter)
                .expect("parameter should have a name");
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
            new_parameter_names.push(name);
            new_parameters.push(new_parameter);
        }
        let list = self.factory_mut().new_node_list(
            source_list.loc(),
            source_list.range(),
            new_parameters,
        );
        (list, new_parameter_names)
    }

    fn transform_async_function_body(
        &mut self,
        node: ast::Node,
        outer_parameter_names: &[ast::Node],
    ) -> ast::Node {
        let inner_parameters = if self.is_simple_parameter_list(node) {
            None
        } else {
            let parameters = {
                let source = self.store_for(node);
                source
                    .source_parameters(node)
                    .map(ast::SourceNodeListInput::from_source)
            };
            self.visit_parameters_input(parameters)
        };

        let is_arrow = self.store_for(node).kind(node) == ast::Kind::ArrowFunction;
        let saved_lexical_arguments = self.lexical_arguments;
        let capture_lexical_arguments = self.lexical_arguments.binding.is_none();
        if capture_lexical_arguments {
            let binding = self.emit_context.factory.new_unique_name("arguments");
            self.lexical_arguments = LexicalArguments {
                binding: Some(binding),
                used: false,
            };
        }

        let arguments_expression = inner_parameters.map(|_| {
            if is_arrow {
                self.forwarded_arrow_arguments_expression(node, outer_parameter_names)
            } else {
                self.factory_mut().new_identifier("arguments")
            }
        });

        let saved_names = self.enclosing_function_parameter_names.take();
        let mut names = HashSet::new();
        let parameters = {
            let source = self.store_for(node);
            source
                .source_parameters(node)
                .expect("async function parameters should exist")
                .iter()
                .collect::<Vec<_>>()
        };
        for parameter in parameters {
            self.record_declaration_name(parameter, &mut names);
        }
        self.enclosing_function_parameter_names = Some(names);

        let body_node = self
            .store_for(node)
            .body(node)
            .expect("async function should have a body");
        let saved_super_state = self.super_access_state.clone();
        let saved_super_binding = self.super_binding;
        let saved_super_index_binding = self.super_index_binding;
        if !is_arrow {
            self.super_access_state = Some(self.new_super_state_for_body(Some(body_node)));
            self.super_binding = Some(self.new_super_binding("_super"));
            self.super_index_binding = Some(self.new_super_binding("_superIndex"));
        }

        let has_lexical_this = self.in_has_lexical_this_context();
        let mut async_body = self
            .with_super_substitution(|this| this.transform_async_function_body_worker(body_node));
        let statements = self
            .factory()
            .store()
            .source_statements(async_body)
            .expect("async body should be a block")
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

        let result = if !is_arrow {
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
            if capture_lexical_arguments && self.lexical_arguments.used {
                let capture = self.create_capture_arguments_statement();
                self.emit_context.add_initialization_statement(capture);
            }
            let awaiter = self.emit_context.factory.new_awaiter_helper(
                has_lexical_this,
                arguments_expression,
                inner_parameters,
                async_body,
            );
            let return_statement = self.factory_mut().new_return_statement(awaiter);
            let mut statements = self.emit_context.end_variable_environment();
            statements.push(return_statement);
            let statements = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                statements,
            );
            let block = self.factory_mut().new_block(statements, true);
            if let Some(body) = self.store_for(node).body(node) {
                let loc = self.store_for(body).loc(body);
                self.factory_mut().place_emit_synthetic_node(block, loc);
            }
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
                    self.emit_context.add_emit_helper(
                        &block,
                        std::slice::from_ref(&printer::ASYNC_SUPER_HELPER),
                    );
                }
            }
            block
        } else {
            let awaiter = self.emit_context.factory.new_awaiter_helper(
                has_lexical_this,
                arguments_expression,
                inner_parameters,
                async_body,
            );
            if capture_lexical_arguments && self.lexical_arguments.used {
                let block = self.convert_to_function_block(awaiter);
                let capture = self.create_capture_arguments_statement();
                let mut statements = vec![capture];
                statements.extend(
                    self.factory()
                        .store()
                        .source_statements(block)
                        .expect("converted block should have statements")
                        .iter()
                        .collect::<Vec<_>>(),
                );
                let statements = self.new_node_list_like_block(block, statements);
                let multi_line = self.store_for(block).multi_line(block).unwrap_or(true);
                self.factory_mut()
                    .update_block(block, statements, multi_line)
            } else {
                awaiter
            }
        };

        self.enclosing_function_parameter_names = saved_names;
        if !is_arrow {
            self.super_access_state = saved_super_state;
            self.super_binding = saved_super_binding;
            self.super_index_binding = saved_super_index_binding;
            self.lexical_arguments = saved_lexical_arguments;
        } else if capture_lexical_arguments && !self.lexical_arguments.used {
            self.lexical_arguments = saved_lexical_arguments;
        } else if capture_lexical_arguments {
            self.lexical_arguments.used = false;
        }
        result
    }

    fn forwarded_arrow_arguments_expression(
        &mut self,
        node: ast::Node,
        outer_parameter_names: &[ast::Node],
    ) -> ast::Node {
        let mut bindings = Vec::new();
        let parameters = {
            let source = self.store_for(node);
            source
                .source_parameters(node)
                .expect("arrow parameters should exist")
                .iter()
                .collect::<Vec<_>>()
        };
        for (idx, parameter) in parameters.iter().enumerate() {
            if idx >= outer_parameter_names.len() {
                break;
            }
            let outer_name = outer_parameter_names[idx];
            let source = self.store_for(*parameter);
            if source.initializer(*parameter).is_some()
                || source.dot_dot_dot_token(*parameter).is_some()
            {
                let spread = self.factory_mut().new_spread_element(outer_name);
                bindings.push(spread);
                break;
            }
            bindings.push(outer_name);
        }
        let elements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            bindings,
        );
        self.factory_mut()
            .new_array_literal_expression(elements, false)
    }

    fn transform_async_function_body_worker(&mut self, body: ast::Node) -> ast::Node {
        if ast::is_block(self.store_for(body), body) {
            let (statements_input, multi_line) = {
                let source = self.store_for(body);
                (
                    source
                        .source_statements(body)
                        .map(ast::SourceNodeListInput::from_source),
                    source.multi_line(body).unwrap_or(true),
                )
            };
            let statements = self.visit_async_body_nodes(statements_input);
            return if self.is_factory_node(body) {
                self.factory_mut().update_block(
                    body,
                    statements.expect("block statements should exist"),
                    multi_line,
                )
            } else {
                let source = self.source;
                self.factory_mut().update_block_from_store(
                    source,
                    body,
                    statements.expect("block statements should exist"),
                    multi_line,
                )
            };
        }
        let visited = self.visit_async_body_node(body);
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

    fn convert_to_function_block(&mut self, node: ast::Node) -> ast::Node {
        if self.store_for(node).kind(node) == ast::Kind::Block {
            return node;
        }
        let return_statement = self.factory_mut().new_return_statement(node);
        let loc = self.store_for(node).loc(node);
        self.factory_mut()
            .place_emit_synthetic_node(return_statement, loc);
        self.emit_context.set_original(&return_statement, &node);
        let statements = self
            .factory_mut()
            .new_node_list(loc, loc, vec![return_statement]);
        let block = self.factory_mut().new_block(statements, true);
        self.factory_mut().place_emit_synthetic_node(block, loc);
        block
    }

    fn visit_async_body_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        if self.is_node_with_possible_hoisted_declaration(node) {
            match self.store_for(node).kind(node) {
                ast::Kind::VariableStatement => {
                    return self.visit_variable_statement_in_async_body(node);
                }
                ast::Kind::ForStatement => {
                    return Some(self.visit_for_statement_in_async_body(node));
                }
                ast::Kind::ForInStatement => {
                    return Some(self.visit_for_in_statement_in_async_body(node));
                }
                ast::Kind::ForOfStatement => {
                    return Some(self.visit_for_of_statement_in_async_body(node));
                }
                ast::Kind::CatchClause => return Some(self.visit_catch_clause_in_async_body(node)),
                ast::Kind::Block => return Some(self.visit_block_in_async_body(node)),
                ast::Kind::TryStatement => {
                    return Some(self.visit_try_statement_in_async_body(node));
                }
                ast::Kind::SwitchStatement
                | ast::Kind::CaseBlock
                | ast::Kind::CaseClause
                | ast::Kind::DefaultClause
                | ast::Kind::DoStatement
                | ast::Kind::WhileStatement
                | ast::Kind::IfStatement
                | ast::Kind::WithStatement
                | ast::Kind::LabeledStatement => {
                    return Some(
                        self.with_async_body_visitor(|this| this.generated_visit_each_child(&node)),
                    );
                }
                _ => {}
            }
        }
        self.visit(&node)
    }

    fn visit_async_body_nodes(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let source_list = nodes.clone();
        let mut visited = Vec::with_capacity(source_list.len());
        let mut changed = false;
        for node in source_list.iter() {
            let result = self.visit_async_body_node(node);
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
            Some(nodes.as_node_list())
        }
    }

    fn visit_block_in_async_body(&mut self, node: ast::Node) -> ast::Node {
        let (statements_input, multi_line) = {
            let source = self.store_for(node);
            (
                source
                    .source_statements(node)
                    .map(ast::SourceNodeListInput::from_source),
                source.multi_line(node).unwrap_or(true),
            )
        };
        let statements = self.visit_async_body_nodes(statements_input);
        if self.is_factory_node(node) {
            self.factory_mut().update_block(
                node,
                statements.expect("block statements should exist"),
                multi_line,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_block_from_store(
                source,
                node,
                statements.expect("block statements should exist"),
                multi_line,
            )
        }
    }

    fn visit_try_statement_in_async_body(&mut self, node: ast::Node) -> ast::Node {
        let (try_block, catch_clause, finally_block) = {
            let source = self.store_for(node);
            (
                source.try_block(node),
                source.catch_clause(node),
                source.finally_block(node),
            )
        };
        let try_block = try_block.map(|block| self.visit_block_in_async_body(block));
        let catch_clause =
            catch_clause.and_then(|catch_clause| self.visit_async_body_node(catch_clause));
        let finally_block = finally_block.map(|block| self.visit_block_in_async_body(block));
        if self.is_factory_node(node) {
            self.factory_mut()
                .update_try_statement(node, try_block, catch_clause, finally_block)
        } else {
            let source = self.source;
            self.factory_mut().update_try_statement_from_store(
                source,
                node,
                try_block,
                catch_clause,
                finally_block,
            )
        }
    }

    fn visit_catch_clause_in_async_body(&mut self, node: ast::Node) -> ast::Node {
        let mut catch_names = HashSet::new();
        if let Some(variable) = self.store_for(node).variable_declaration(node) {
            self.record_declaration_name(variable, &mut catch_names);
        }
        let unshadowed = self
            .enclosing_function_parameter_names
            .as_ref()
            .and_then(|names| {
                let mut clone = names.clone();
                let mut changed = false;
                for name in &catch_names {
                    if clone.remove(name) {
                        changed = true;
                    }
                }
                changed.then_some(clone)
            });
        if let Some(unshadowed) = unshadowed {
            let saved = self.enclosing_function_parameter_names.replace(unshadowed);
            let result =
                self.with_async_body_visitor(|this| this.generated_visit_each_child(&node));
            self.enclosing_function_parameter_names = saved;
            result
        } else {
            self.with_async_body_visitor(|this| this.generated_visit_each_child(&node))
        }
    }

    fn visit_variable_statement_in_async_body(&mut self, node: ast::Node) -> Option<ast::Node> {
        let decl_list = self
            .store_for(node)
            .declaration_list(node)
            .expect("variable statement should have declaration list");
        if self.is_variable_declaration_list_with_colliding_name(decl_list) {
            let expression =
                self.visit_variable_declaration_list_with_colliding_names(decl_list, false);
            return expression
                .map(|expression| self.factory_mut().new_expression_statement(expression));
        }
        Some(self.generated_visit_each_child(&node))
    }

    fn visit_for_in_statement_in_async_body(&mut self, node: ast::Node) -> ast::Node {
        let (initializer, expression, statement) = {
            let source = self.store_for(node);
            (
                source.initializer(node),
                source.expression(node),
                source.statement(node),
            )
        };
        let initializer = initializer.and_then(|initializer| {
            if self.is_variable_declaration_list_with_colliding_name(initializer) {
                self.visit_variable_declaration_list_with_colliding_names(initializer, true)
            } else {
                self.visit_node(Some(initializer))
            }
        });
        let expression = expression.and_then(|node| self.visit_node(Some(node)));
        let statement =
            self.with_async_body_visitor(|this| this.visit_embedded_statement(statement));
        if self.is_factory_node(node) {
            self.factory_mut().update_for_in_or_of_statement(
                node,
                None::<ast::Node>,
                initializer,
                expression,
                statement,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_for_in_or_of_statement_from_store(
                source,
                node,
                None::<ast::Node>,
                initializer,
                expression,
                statement,
            )
        }
    }

    fn visit_for_of_statement_in_async_body(&mut self, node: ast::Node) -> ast::Node {
        let (initializer, await_modifier, expression, statement) = {
            let source = self.store_for(node);
            (
                source.initializer(node),
                source.await_modifier(node),
                source.expression(node),
                source.statement(node),
            )
        };
        let initializer = initializer.and_then(|initializer| {
            if self.is_variable_declaration_list_with_colliding_name(initializer) {
                self.visit_variable_declaration_list_with_colliding_names(initializer, true)
            } else {
                self.visit_node(Some(initializer))
            }
        });
        let await_modifier = await_modifier.and_then(|node| self.visit_node(Some(node)));
        let expression = expression.and_then(|node| self.visit_node(Some(node)));
        let statement =
            self.with_async_body_visitor(|this| this.visit_embedded_statement(statement));
        if self.is_factory_node(node) {
            self.factory_mut().update_for_in_or_of_statement(
                node,
                await_modifier,
                initializer,
                expression,
                statement,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_for_in_or_of_statement_from_store(
                source,
                node,
                await_modifier,
                initializer,
                expression,
                statement,
            )
        }
    }

    fn visit_for_statement_in_async_body(&mut self, node: ast::Node) -> ast::Node {
        let (initializer, condition, incrementor, statement) = {
            let source = self.store_for(node);
            (
                source.initializer(node),
                source.condition(node),
                source.incrementor(node),
                source.statement(node),
            )
        };
        let initializer = initializer.and_then(|initializer| {
            if self.is_variable_declaration_list_with_colliding_name(initializer) {
                self.visit_variable_declaration_list_with_colliding_names(initializer, false)
            } else {
                self.visit_node(Some(initializer))
            }
        });
        let condition = condition.and_then(|node| self.visit_node(Some(node)));
        let incrementor = incrementor.and_then(|node| self.visit_node(Some(node)));
        let statement =
            self.with_async_body_visitor(|this| this.visit_embedded_statement(statement));
        if self.is_factory_node(node) {
            self.factory_mut().update_for_statement(
                node,
                initializer,
                condition,
                incrementor,
                statement,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_for_statement_from_store(
                source,
                node,
                initializer,
                condition,
                incrementor,
                statement,
            )
        }
    }

    fn record_declaration_name(&self, node: ast::Node, names: &mut HashSet<String>) {
        let store = self.store_for(node);
        let Some(name) = store.name(node) else {
            return;
        };
        if ast::is_identifier(store, name) {
            names.insert(store.text(name).to_string());
        } else if ast::is_binding_pattern(store, name)
            && let Some(elements) = store.source_elements(name)
        {
            for element in elements.iter() {
                if !ast::is_omitted_expression(store, element) {
                    self.record_declaration_name(element, names);
                }
            }
        }
    }

    fn is_variable_declaration_list_with_colliding_name(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        ast::is_variable_declaration_list(store, node)
            && !store.flags(node).intersects(ast::NodeFlags::BLOCK_SCOPED)
            && store.source_declarations(node).is_some_and(|decls| {
                decls
                    .iter()
                    .any(|decl| self.collides_with_parameter_name(decl))
            })
    }

    fn visit_variable_declaration_list_with_colliding_names(
        &mut self,
        node: ast::Node,
        has_receiver: bool,
    ) -> Option<ast::Node> {
        self.hoist_variable_declaration_list(node);
        let store = self.store_for(node);
        let declarations = store
            .source_declarations(node)
            .expect("variable declaration list should have declarations")
            .iter()
            .collect::<Vec<_>>();
        let variables = declarations
            .iter()
            .copied()
            .filter(|decl| self.store_for(*decl).initializer(*decl).is_some())
            .collect::<Vec<_>>();
        if variables.is_empty() {
            if has_receiver {
                let first = declarations[0];
                let (name, is_binding_pattern) = {
                    let store = self.store_for(first);
                    let name = store.name(first).expect("declaration should have name");
                    (name, ast::is_binding_pattern(store, name))
                };
                let target = if is_binding_pattern {
                    self.convert_binding_pattern_to_assignment_pattern(name)
                } else {
                    self.preserve_node(name)
                };
                return self.visit_node(Some(target));
            }
            return None;
        }
        let mut expressions = Vec::new();
        for variable in variables {
            expressions.push(self.transform_initialized_variable(variable));
        }
        self.emit_context.factory.inline_expressions(&expressions)
    }

    fn hoist_variable_declaration_list(&mut self, node: ast::Node) {
        let declarations = self
            .store_for(node)
            .source_declarations(node)
            .map(|decls| decls.iter().collect::<Vec<_>>());
        if let Some(declarations) = declarations {
            for declaration in declarations {
                self.hoist_variable(declaration);
            }
        }
    }

    fn hoist_variable(&mut self, node: ast::Node) {
        let Some((name, is_identifier, elements)) = ({
            let store = self.store_for(node);
            store.name(node).map(|name| {
                let elements = if ast::is_binding_pattern(store, name) {
                    store.source_elements(name).map(|elements| {
                        elements
                            .iter()
                            .filter(|element| !ast::is_omitted_expression(store, *element))
                            .collect::<Vec<_>>()
                    })
                } else {
                    None
                };
                (name, ast::is_identifier(store, name), elements)
            })
        }) else {
            return;
        };
        if is_identifier {
            let name = self.preserve_node(name);
            self.emit_context.add_variable_declaration(name);
        } else if let Some(elements) = elements {
            for element in elements {
                self.hoist_variable(element);
            }
        }
    }

    fn transform_initialized_variable(&mut self, node: ast::Node) -> ast::Node {
        let (name, is_binding_pattern, initializer, loc) = {
            let store = self.store_for(node);
            let name = store
                .name(node)
                .expect("variable declaration should have name");
            let initializer = store
                .initializer(node)
                .expect("initialized variable should have initializer");
            (
                name,
                ast::is_binding_pattern(store, name),
                initializer,
                store.loc(node),
            )
        };
        let target = if is_binding_pattern {
            self.convert_binding_pattern_to_assignment_pattern(name)
        } else {
            self.preserve_node(name)
        };
        let initializer = self.preserve_node(initializer);
        let assignment = self
            .emit_context
            .factory
            .new_assignment_expression(target, initializer);
        self.emit_context.set_source_map_range(&assignment, loc);
        self.visit_node(Some(assignment))
            .expect("assignment expression should not be removed")
    }

    fn collides_with_parameter_name(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        let Some(name) = store.name(node) else {
            return false;
        };
        if ast::is_identifier(store, name) {
            return self
                .enclosing_function_parameter_names
                .as_ref()
                .is_some_and(|names| names.contains(&store.text(name)));
        }
        if ast::is_binding_pattern(store, name)
            && let Some(elements) = store.source_elements(name)
        {
            return elements.iter().any(|element| {
                !ast::is_omitted_expression(store, element)
                    && self.collides_with_parameter_name(element)
            });
        }
        false
    }

    fn create_capture_arguments_statement(&mut self) -> ast::Node {
        let binding = self
            .lexical_arguments
            .binding
            .expect("lexical arguments binding should exist");
        let arguments = self.factory_mut().new_identifier("arguments");
        let variable = self.factory_mut().new_variable_declaration(
            binding,
            None::<ast::Node>,
            None::<ast::Node>,
            arguments,
        );
        let declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![variable],
        );
        let declaration_list = self
            .factory_mut()
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        let statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, declaration_list);
        self.emit_context.set_emit_flags(
            &statement,
            printer::EF_START_ON_NEW_LINE | printer::EF_CUSTOM_PROLOGUE,
        );
        statement
    }

    fn is_simple_parameter_list(&self, node: ast::Node) -> bool {
        self.store_for(node)
            .source_parameters(node)
            .is_none_or(|parameters| {
                parameters.iter().all(|parameter| {
                    let source = self.store_for(parameter);
                    source.initializer(parameter).is_none()
                        && source
                            .name(parameter)
                            .is_some_and(|name| ast::is_identifier(self.store_for(name), name))
                })
            })
    }

    fn is_node_with_possible_hoisted_declaration(&self, node: ast::Node) -> bool {
        matches!(
            self.store_for(node).kind(node),
            ast::Kind::Block
                | ast::Kind::VariableStatement
                | ast::Kind::WithStatement
                | ast::Kind::IfStatement
                | ast::Kind::SwitchStatement
                | ast::Kind::CaseBlock
                | ast::Kind::CaseClause
                | ast::Kind::DefaultClause
                | ast::Kind::LabeledStatement
                | ast::Kind::ForStatement
                | ast::Kind::ForInStatement
                | ast::Kind::ForOfStatement
                | ast::Kind::DoStatement
                | ast::Kind::WhileStatement
                | ast::Kind::TryStatement
                | ast::Kind::CatchClause
        )
    }

    fn is_name_of_property_access_or_assignment(&self, node: ast::Node) -> bool {
        self.parent_node.is_some_and(|parent| {
            matches!(
                self.store_for(parent).kind(parent),
                ast::Kind::PropertyAccessExpression | ast::Kind::PropertyAssignment
            ) && self.store_for(parent).name(parent) == Some(node)
        })
    }

    fn convert_binding_pattern_to_assignment_pattern(&mut self, pattern: ast::Node) -> ast::Node {
        let kind = self.store_for(pattern).kind(pattern);
        match kind {
            ast::Kind::ArrayBindingPattern => {
                let mut elements = Vec::new();
                let (source_elements, pattern_loc) = {
                    let store = self.store_for(pattern);
                    (
                        store.source_elements(pattern).map(|source_elements| {
                            (
                                source_elements.loc(),
                                source_elements.range(),
                                source_elements.iter().collect::<Vec<_>>(),
                            )
                        }),
                        store.loc(pattern),
                    )
                };
                if let Some((loc, range, source_elements)) = source_elements {
                    for element in source_elements {
                        elements.push(
                            self.convert_binding_element_to_array_assignment_element(element),
                        );
                    }
                    let list = self.factory_mut().new_node_list(loc, range, elements);
                    let array = self.factory_mut().new_array_literal_expression(list, false);
                    self.emit_context.set_original(&array, &pattern);
                    self.emit_context.set_source_map_range(&array, pattern_loc);
                    array
                } else {
                    let list = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        elements,
                    );
                    self.factory_mut().new_array_literal_expression(list, false)
                }
            }
            ast::Kind::ObjectBindingPattern => {
                let mut properties = Vec::new();
                let (source_elements, pattern_loc) = {
                    let store = self.store_for(pattern);
                    (
                        store.source_elements(pattern).map(|source_elements| {
                            (
                                source_elements.loc(),
                                source_elements.range(),
                                source_elements.iter().collect::<Vec<_>>(),
                            )
                        }),
                        store.loc(pattern),
                    )
                };
                if let Some((loc, range, source_elements)) = source_elements {
                    for element in source_elements {
                        properties.push(
                            self.convert_binding_element_to_object_assignment_element(element),
                        );
                    }
                    let list = self.factory_mut().new_node_list(loc, range, properties);
                    let object = self
                        .factory_mut()
                        .new_object_literal_expression(list, false);
                    self.emit_context.set_original(&object, &pattern);
                    self.emit_context.set_source_map_range(&object, pattern_loc);
                    object
                } else {
                    let list = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        properties,
                    );
                    self.factory_mut()
                        .new_object_literal_expression(list, false)
                }
            }
            _ => panic!("unknown binding pattern"),
        }
    }

    fn convert_binding_element_to_array_assignment_element(
        &mut self,
        element: ast::Node,
    ) -> ast::Node {
        let (name, has_dot_dot_dot, initializer, loc) = {
            let store = self.store_for(element);
            (
                store.name(element),
                store.dot_dot_dot_token(element).is_some(),
                store.initializer(element),
                store.loc(element),
            )
        };
        let Some(name) = name else {
            let omitted = self.factory_mut().new_omitted_expression();
            self.emit_context.set_original(&omitted, &element);
            self.emit_context.set_source_map_range(&omitted, loc);
            return omitted;
        };
        if has_dot_dot_dot {
            let name = self.preserve_node(name);
            let spread = self.factory_mut().new_spread_element(name);
            self.emit_context.set_original(&spread, &element);
            self.emit_context.set_source_map_range(&spread, loc);
            return spread;
        }
        let mut expression = self.convert_binding_name_to_assignment_element_target(name);
        if let Some(initializer) = initializer {
            let initializer = self.preserve_node(initializer);
            expression = self
                .emit_context
                .factory
                .new_assignment_expression(expression, initializer);
            self.emit_context.set_original(&expression, &element);
            self.emit_context.set_source_map_range(&expression, loc);
        }
        expression
    }

    fn convert_binding_element_to_object_assignment_element(
        &mut self,
        element: ast::Node,
    ) -> ast::Node {
        let (name, has_dot_dot_dot, property_name, initializer, loc) = {
            let store = self.store_for(element);
            (
                store.name(element),
                store.dot_dot_dot_token(element).is_some(),
                store.property_name(element),
                store.initializer(element),
                store.loc(element),
            )
        };
        let preserved_name = name.map(|name| self.preserve_node(name));
        if has_dot_dot_dot {
            let spread = self.factory_mut().new_spread_assignment(preserved_name);
            self.emit_context.set_original(&spread, &element);
            self.emit_context.set_source_map_range(&spread, loc);
            return spread;
        }
        if let Some(property_name) = property_name {
            let original_name = name.expect("binding element should have name");
            let mut expression =
                self.convert_binding_name_to_assignment_element_target(original_name);
            if let Some(initializer) = initializer {
                let initializer = self.preserve_node(initializer);
                expression = self
                    .emit_context
                    .factory
                    .new_assignment_expression(expression, initializer);
            }
            let property_name = self.preserve_node(property_name);
            let assignment = self.factory_mut().new_property_assignment(
                None::<ast::ModifierList>,
                property_name,
                None::<ast::Node>,
                None::<ast::Node>,
                expression,
            );
            self.emit_context.set_original(&assignment, &element);
            self.emit_context.set_source_map_range(&assignment, loc);
            return assignment;
        }
        let equals_token = if initializer.is_some() {
            Some(self.factory_mut().new_token(ast::Kind::EqualsToken))
        } else {
            None
        };
        let initializer = initializer.map(|node| self.preserve_node(node));
        let assignment = self.factory_mut().new_shorthand_property_assignment(
            None::<ast::ModifierList>,
            preserved_name,
            None::<ast::Node>,
            None::<ast::Node>,
            equals_token,
            initializer,
        );
        self.emit_context.set_original(&assignment, &element);
        self.emit_context.set_source_map_range(&assignment, loc);
        assignment
    }

    fn convert_binding_name_to_assignment_element_target(&mut self, name: ast::Node) -> ast::Node {
        if ast::is_binding_pattern(self.store_for(name), name) {
            self.convert_binding_pattern_to_assignment_pattern(name)
        } else {
            self.preserve_node(name)
        }
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

impl<'source> ast::AstVisitEachChildRuntime<'source> for AsyncRuntime<'_, 'source> {
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn source_store_for_node(&self, node: ast::Node) -> &ast::AstStore {
        self.emit_context.store_for_node(node)
    }

    fn source_store_for_store_id(&self, store_id: ast::StoreId) -> &ast::AstStore {
        self.emit_context.store_for_store_id(store_id)
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
            let result = if self.visiting_async_body {
                self.visit_async_body_node(node)
            } else {
                self.visit(&node)
            };
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
            Some(nodes.as_node_list())
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
            Some(modifiers.as_modifier_list())
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
            Some(nodes.as_node_list())
        }
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        match node {
            Some(node) => {
                let visited = if self.visiting_async_body {
                    self.visit_async_body_node(node)
                } else {
                    self.visit(&node)
                };
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for AsyncRuntime<'_, 'source> {}
