use std::collections::HashSet;

use ts_ast as ast;
use ts_ast::{AstGeneratedVisitEachChild as _, AstVisitEachChildRuntime as _};
use ts_core as core;
use ts_printer as printer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectRestSpreadAction {
    Keep,
    VisitChildren,
    TransformSourceFile,
    TransformObjectLiteral,
    TransformBinaryExpression,
    MarkExpressionResultUnused,
    TransformForOfStatement,
    TrackExportedVariableStatement,
    TransformVariableDeclaration,
    TransformCatchClause,
    TransformParameter,
    TransformFunctionLike,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlattenLevel {
    ObjectRest,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectLiteralChunkKind {
    ObjectProperties,
    SpreadExpression,
    EmptyObjectPrefix,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectRestSpreadFacts {
    pub subtree_contains_object_rest_or_spread: bool,
    pub has_parameter_scope_pending: bool,
    pub expression_result_is_unused: bool,
    pub is_exported_variable_statement: bool,
    pub name_is_binding_pattern: bool,
    pub initializer_present: bool,
    pub contains_object_rest_or_spread: bool,
    pub is_destructuring_assignment: bool,
    pub is_comma_expression: bool,
    pub for_initializer_contains_rest_or_spread: bool,
}

pub fn object_rest_spread_action_for_kind(
    kind: ast::Kind,
    facts: ObjectRestSpreadFacts,
) -> ObjectRestSpreadAction {
    if !facts.subtree_contains_object_rest_or_spread && !facts.has_parameter_scope_pending {
        return ObjectRestSpreadAction::Keep;
    }

    match kind {
        ast::Kind::SourceFile => ObjectRestSpreadAction::TransformSourceFile,
        ast::Kind::ObjectLiteralExpression => ObjectRestSpreadAction::TransformObjectLiteral,
        ast::Kind::BinaryExpression => ObjectRestSpreadAction::TransformBinaryExpression,
        ast::Kind::ExpressionStatement => ObjectRestSpreadAction::MarkExpressionResultUnused,
        ast::Kind::ParenthesizedExpression => ObjectRestSpreadAction::VisitChildren,
        ast::Kind::ForOfStatement => ObjectRestSpreadAction::TransformForOfStatement,
        ast::Kind::VariableStatement if facts.is_exported_variable_statement => {
            ObjectRestSpreadAction::TrackExportedVariableStatement
        }
        ast::Kind::VariableDeclaration => ObjectRestSpreadAction::TransformVariableDeclaration,
        ast::Kind::CatchClause => ObjectRestSpreadAction::TransformCatchClause,
        ast::Kind::Parameter => ObjectRestSpreadAction::TransformParameter,
        ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::MethodDeclaration
        | ast::Kind::FunctionDeclaration
        | ast::Kind::ArrowFunction
        | ast::Kind::FunctionExpression => ObjectRestSpreadAction::TransformFunctionLike,
        _ => ObjectRestSpreadAction::VisitChildren,
    }
}

pub fn variable_declaration_flatten_level(facts: ObjectRestSpreadFacts) -> Option<FlattenLevel> {
    (facts.name_is_binding_pattern && facts.contains_object_rest_or_spread)
        .then_some(FlattenLevel::ObjectRest)
}

pub fn binary_expression_flatten_level(facts: ObjectRestSpreadFacts) -> Option<FlattenLevel> {
    (facts.is_destructuring_assignment && facts.contains_object_rest_or_spread)
        .then_some(FlattenLevel::ObjectRest)
}

pub fn destructuring_assignment_needs_value(expression_result_is_unused: bool) -> bool {
    !expression_result_is_unused
}

pub fn parameter_with_preceding_rest_or_spread_needs_generated_name(
    name_is_binding_pattern: bool,
) -> bool {
    name_is_binding_pattern
}

pub fn parameter_object_rest_flatten_level(is_preceding_parameter: bool) -> FlattenLevel {
    if is_preceding_parameter {
        FlattenLevel::All
    } else {
        FlattenLevel::ObjectRest
    }
}

pub fn for_of_needs_temp_binding(facts: ObjectRestSpreadFacts) -> bool {
    facts.for_initializer_contains_rest_or_spread
}

pub fn chunk_object_literal_element_kinds(
    elements_are_spread: &[bool],
) -> Vec<ObjectLiteralChunkKind> {
    let mut chunks = Vec::new();
    let mut pending_properties = false;
    for is_spread in elements_are_spread {
        if *is_spread {
            if pending_properties {
                chunks.push(ObjectLiteralChunkKind::ObjectProperties);
                pending_properties = false;
            }
            chunks.push(ObjectLiteralChunkKind::SpreadExpression);
        } else {
            pending_properties = true;
        }
    }
    if pending_properties {
        chunks.push(ObjectLiteralChunkKind::ObjectProperties);
    }
    if matches!(
        chunks.first(),
        Some(ObjectLiteralChunkKind::SpreadExpression)
    ) {
        chunks.insert(0, ObjectLiteralChunkKind::EmptyObjectPrefix);
    }
    chunks
}

pub fn visit_source_file_root(
    source_file: &ast::SourceFile,
    root: ast::Node,
    emit_context: &mut printer::EmitContext,
    compiler_options: &core::CompilerOptions,
) -> ast::Node {
    if source_file.is_declaration_file() {
        return root;
    }

    let mut runtime = ObjectRestSpreadRuntime {
        source: source_file.store(),
        emit_context,
        import_state: ast::AstImportState::new(),
        compiler_options,
        in_exported_variable_statement: false,
        expression_result_is_unused: false,
        parameters_with_preceding_object_rest_or_spread: None,
    };
    let root = runtime.visit_node(Some(root)).unwrap_or(root);
    runtime.emit_context.add_requested_emit_helpers(&root);
    root
}

struct PendingDecl {
    pending_expressions: Vec<ast::Node>,
    name: ast::Node,
    value: ast::Node,
    location: core::TextRange,
    original: Option<ast::Node>,
}

struct BindingFlattener<'a, 'ctx, 'source> {
    runtime: &'a mut ObjectRestSpreadRuntime<'ctx, 'source>,
    level: FlattenLevel,
    expressions: Vec<ast::Node>,
    declarations: Vec<PendingDecl>,
    has_transformed_prior_element: bool,
    hoist_temp_variables: bool,
}

impl<'a, 'ctx, 'source> BindingFlattener<'a, 'ctx, 'source> {
    fn new(
        runtime: &'a mut ObjectRestSpreadRuntime<'ctx, 'source>,
        level: FlattenLevel,
        hoist_temp_variables: bool,
    ) -> Self {
        Self {
            runtime,
            level,
            expressions: Vec::new(),
            declarations: Vec::new(),
            has_transformed_prior_element: false,
            hoist_temp_variables,
        }
    }

    fn flatten(
        mut self,
        mut node: ast::Node,
        rval: Option<ast::Node>,
        skip_initializer: bool,
    ) -> Option<ast::Node> {
        if self.runtime.store_for(node).kind(node) == ast::Kind::VariableDeclaration {
            let (
                initializer,
                initializer_loc,
                name,
                initializer_assigns_to_name,
                contains_non_literal_computed_name,
            ) = {
                let source = self.runtime.store_for(node);
                let initializer = source.initializer(node);
                let initializer_assigns_to_name = initializer.is_some_and(|initializer| {
                    ast::is_identifier(source, initializer)
                        && crate::destructuring::binding_or_assignment_element_assigns_to_name(
                            source,
                            node,
                            &source.text(initializer),
                        )
                });
                (
                    initializer,
                    initializer.map(|initializer| source.loc(initializer)),
                    source.name(node),
                    initializer_assigns_to_name,
                    crate::destructuring::binding_or_assignment_element_contains_non_literal_computed_name(
                        source, node,
                    ),
                )
            };
            if let (Some(initializer), Some(initializer_loc)) = (initializer, initializer_loc)
                && (initializer_assigns_to_name || contains_non_literal_computed_name)
            {
                let initializer = self
                    .runtime
                    .visit_node(Some(initializer))
                    .expect("variable declaration initializer should visit to an expression");
                let initializer = self.ensure_identifier(initializer, false, initializer_loc);
                if node.store_id() == self.runtime.factory().store().store_id() {
                    node = self.runtime.factory_mut().update_variable_declaration(
                        node,
                        name,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        initializer,
                    );
                } else {
                    let source = self.runtime.source;
                    node = self
                        .runtime
                        .factory_mut()
                        .update_variable_declaration_from_store(
                            source,
                            node,
                            name,
                            None::<ast::Node>,
                            None::<ast::Node>,
                            initializer,
                        );
                }
            }
        }
        let source = self.runtime.store_for(node);
        self.flatten_binding_or_assignment_element(node, rval, source.loc(node), skip_initializer);

        if !self.expressions.is_empty() {
            let temp = self.runtime.emit_context.factory.new_temp_variable();
            self.runtime.emit_context.add_variable_declaration(temp);
            let last = self
                .declarations
                .last_mut()
                .expect("pending expressions require a declaration receiver");
            let assignment = self
                .runtime
                .emit_context
                .factory
                .new_assignment_expression(temp, last.value);
            last.pending_expressions.push(assignment);
            last.pending_expressions.append(&mut self.expressions);
            last.value = temp;
        }

        let mut declarations = Vec::new();
        for pending in self.declarations {
            let value = if pending.pending_expressions.is_empty() {
                pending.value
            } else {
                let mut expressions = pending.pending_expressions;
                expressions.push(pending.value);
                self.runtime
                    .emit_context
                    .factory
                    .inline_expressions(&expressions)
                    .expect("pending declaration expression list should not be empty")
            };
            let declaration = self.runtime.factory_mut().new_variable_declaration(
                pending.name,
                None::<ast::Node>,
                None::<ast::Node>,
                value,
            );
            self.runtime
                .factory_mut()
                .place_emit_synthetic_node(declaration, pending.location);
            if let Some(original) = pending.original {
                self.runtime
                    .emit_context
                    .set_original(&declaration, &original);
            }
            declarations.push(declaration);
        }

        match declarations.len() {
            0 => None,
            1 => Some(declarations[0]),
            _ => Some(self.runtime.factory_mut().new_syntax_list(declarations)),
        }
    }

    fn ensure_identifier(
        &mut self,
        value: ast::Node,
        reuse_identifier_expressions: bool,
        location: core::TextRange,
    ) -> ast::Node {
        let store = self.runtime.store_for(value);
        if reuse_identifier_expressions && ast::is_identifier(store, value) {
            return value;
        }
        let temp = self.runtime.emit_context.factory.new_temp_variable();
        if self.hoist_temp_variables {
            self.runtime.emit_context.add_variable_declaration(temp);
            let assign = self
                .runtime
                .emit_context
                .factory
                .new_assignment_expression(temp, value);
            self.runtime
                .factory_mut()
                .place_emit_synthetic_node(assign, location);
            self.expressions.push(assign);
        } else {
            self.emit_binding(temp, value, location, None);
        }
        temp
    }

    fn flatten_binding_or_assignment_element(
        &mut self,
        element: ast::Node,
        mut value: Option<ast::Node>,
        location: core::TextRange,
        skip_initializer: bool,
    ) {
        let Some(binding_target) = self.get_target_of_binding_or_assignment_element(element) else {
            return;
        };
        if !skip_initializer {
            if let Some(initializer) =
                self.get_initializer_of_binding_or_assignment_element(element)
            {
                let initializer = self.runtime.visit_node(Some(initializer));
                if let Some(initializer) = initializer {
                    value = if let Some(value) = value {
                        let mut value =
                            self.create_default_value_check(value, initializer, location);
                        if !self.is_simple_inlineable_expression(initializer)
                            && self.is_binding_or_assignment_pattern(binding_target)
                        {
                            value = self.ensure_identifier(value, true, location);
                        }
                        Some(value)
                    } else {
                        Some(initializer)
                    };
                }
            } else if value.is_none() {
                value = Some(self.runtime.emit_context.factory.new_void_zero_expression());
            }
        }

        let value = value.expect("destructuring binding should have a value");
        let target_store = self.runtime.store_for(binding_target);
        match target_store.kind(binding_target) {
            ast::Kind::ObjectBindingPattern => {
                self.flatten_object_binding_pattern(element, binding_target, value, location);
            }
            ast::Kind::ArrayBindingPattern => {
                self.flatten_array_binding_pattern(element, binding_target, value, location);
            }
            _ => {
                let target = self.runtime.preserve_node(binding_target);
                self.emit_binding(target, value, location, Some(element));
            }
        }
    }

    fn flatten_object_binding_pattern(
        &mut self,
        parent: ast::Node,
        pattern: ast::Node,
        mut value: ast::Node,
        location: core::TextRange,
    ) {
        let elements_vec = {
            let source = self.runtime.store_for(pattern);
            let Some(elements) = source.source_elements(pattern) else {
                return;
            };
            elements.iter().collect::<Vec<_>>()
        };
        if elements_vec.len() != 1 {
            let reuse_identifier_expressions =
                !self.is_declaration_binding_element(parent) || !elements_vec.is_empty();
            value = self.ensure_identifier(value, reuse_identifier_expressions, location);
        }
        let mut binding_elements = Vec::new();
        let mut computed_temp_variables = Vec::new();
        for (index, element) in elements_vec.iter().copied().enumerate() {
            let element_loc = self.runtime.store_for(element).loc(element);
            if self
                .runtime
                .store_for(element)
                .dot_dot_dot_token(element)
                .is_none()
            {
                let property_name = ast::try_get_property_name_of_binding_or_assignment_element(
                    self.runtime.store_for(element),
                    element,
                );
                let can_remain_grouped =
                    matches!(self.level, FlattenLevel::ObjectRest | FlattenLevel::All)
                        && !self.element_contains_rest_or_spread(element)
                        && self
                            .get_target_of_binding_or_assignment_element(element)
                            .is_some_and(|target| !self.element_contains_rest_or_spread(target))
                        && property_name.is_none_or(|name| {
                            !ast::is_computed_property_name(self.runtime.store_for(name), name)
                        });
                if can_remain_grouped {
                    if let Some(visited) = self.runtime.visit_node(Some(element)) {
                        binding_elements.push(visited);
                    }
                } else {
                    if !binding_elements.is_empty() {
                        let target = self.create_object_binding_pattern(&mut binding_elements);
                        self.emit_binding(target, value, location, Some(pattern));
                    }
                    let Some(property_name) = property_name else {
                        continue;
                    };
                    let rhs_value = self.create_destructuring_property_access(value, property_name);
                    if ast::is_computed_property_name(
                        self.runtime.store_for(property_name),
                        property_name,
                    ) && let Some(argument) = self
                        .runtime
                        .store_for(rhs_value)
                        .argument_expression(rhs_value)
                    {
                        computed_temp_variables.push(argument);
                    }
                    self.flatten_binding_or_assignment_element(
                        element,
                        Some(rhs_value),
                        element_loc,
                        false,
                    );
                }
            } else if index == elements_vec.len() - 1 {
                if !binding_elements.is_empty() {
                    let target = self.create_object_binding_pattern(&mut binding_elements);
                    self.emit_binding(target, value, location, Some(pattern));
                }
                let pattern_loc = self.runtime.store_for(pattern).loc(pattern);
                let computed_temp_variables = (!computed_temp_variables.is_empty())
                    .then_some(computed_temp_variables.as_slice());
                let rest_value = if pattern.store_id() == self.runtime.factory().store().store_id()
                {
                    self.runtime
                        .emit_context
                        .factory
                        .new_rest_helper_current_store(
                            value,
                            &elements_vec,
                            computed_temp_variables,
                            pattern_loc,
                        )
                } else {
                    self.runtime.emit_context.factory.new_rest_helper(
                        self.runtime.source,
                        value,
                        &elements_vec,
                        computed_temp_variables,
                        pattern_loc,
                    )
                };
                self.flatten_binding_or_assignment_element(
                    element,
                    Some(rest_value),
                    element_loc,
                    false,
                );
            }
        }
        if !binding_elements.is_empty() {
            let target = self.create_object_binding_pattern(&mut binding_elements);
            self.emit_binding(target, value, location, Some(pattern));
        }
    }

    fn flatten_array_binding_pattern(
        &mut self,
        parent: ast::Node,
        pattern: ast::Node,
        mut value: ast::Node,
        location: core::TextRange,
    ) {
        let elements_vec = {
            let source = self.runtime.store_for(pattern);
            let Some(elements) = source.source_elements(pattern) else {
                return;
            };
            elements.iter().collect::<Vec<_>>()
        };
        let all_omitted = elements_vec
            .iter()
            .all(|element| ast::is_omitted_expression(self.runtime.store_for(*element), *element));
        if (elements_vec.len() != 1
            && (!matches!(self.level, FlattenLevel::ObjectRest | FlattenLevel::All)
                || elements_vec.is_empty()))
            || all_omitted
        {
            let reuse_identifier_expressions =
                !self.is_declaration_binding_element(parent) || !elements_vec.is_empty();
            value = self.ensure_identifier(value, reuse_identifier_expressions, location);
        }
        let mut binding_elements = Vec::new();
        let mut rest_containing_elements = Vec::<(ast::Node, ast::Node)>::new();
        for (index, element) in elements_vec.iter().copied().enumerate() {
            let element_loc = self.runtime.store_for(element).loc(element);
            if matches!(self.level, FlattenLevel::ObjectRest | FlattenLevel::All) {
                if self.element_contains_object_rest_or_spread(element)
                    || (self.has_transformed_prior_element
                        && !self.is_simple_binding_or_assignment_element(element))
                {
                    self.has_transformed_prior_element = true;
                    let temp = self.runtime.emit_context.factory.new_temp_variable();
                    if self.hoist_temp_variables {
                        self.runtime.emit_context.add_variable_declaration(temp);
                    }
                    rest_containing_elements.push((temp, element));
                    let binding = self.runtime.factory_mut().new_binding_element(
                        None::<ast::Node>,
                        None::<ast::Node>,
                        temp,
                        None::<ast::Node>,
                    );
                    binding_elements.push(binding);
                } else if let Some(visited) = self.runtime.visit_node(Some(element)) {
                    binding_elements.push(visited);
                }
            } else if ast::is_omitted_expression(self.runtime.store_for(element), element) {
                continue;
            } else if self
                .runtime
                .store_for(element)
                .dot_dot_dot_token(element)
                .is_none()
            {
                let index = self
                    .runtime
                    .factory_mut()
                    .new_numeric_literal(&index.to_string(), ast::TokenFlags::NONE);
                let rhs = self.runtime.factory_mut().new_element_access_expression(
                    value,
                    None::<ast::Node>,
                    index,
                    ast::NodeFlags::NONE,
                );
                self.flatten_binding_or_assignment_element(element, Some(rhs), element_loc, false);
            } else if index == elements_vec.len() - 1 {
                let rhs = self
                    .runtime
                    .emit_context
                    .factory
                    .new_array_slice_call(&value, index as i32);
                self.flatten_binding_or_assignment_element(element, Some(rhs), element_loc, false);
            }
        }
        if !binding_elements.is_empty() {
            let target = self.create_array_binding_pattern(&mut binding_elements);
            self.emit_binding(target, value, location, Some(pattern));
        }
        for (temp, element) in rest_containing_elements {
            self.flatten_binding_or_assignment_element(
                element,
                Some(temp),
                self.runtime.store_for(element).loc(element),
                false,
            );
        }
    }

    fn create_object_binding_pattern(&mut self, elements: &mut Vec<ast::Node>) -> ast::Node {
        let elements = self.runtime.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            std::mem::take(elements),
        );
        self.runtime
            .factory_mut()
            .new_binding_pattern(ast::Kind::ObjectBindingPattern, elements)
    }

    fn create_array_binding_pattern(&mut self, elements: &mut Vec<ast::Node>) -> ast::Node {
        let elements = self.runtime.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            std::mem::take(elements),
        );
        self.runtime
            .factory_mut()
            .new_binding_pattern(ast::Kind::ArrayBindingPattern, elements)
    }

    fn create_default_value_check(
        &mut self,
        value: ast::Node,
        default_value: ast::Node,
        location: core::TextRange,
    ) -> ast::Node {
        let value = self.ensure_identifier(value, true, location);
        let type_check = self
            .runtime
            .emit_context
            .factory
            .new_type_check(&value, "undefined");
        let question = self
            .runtime
            .factory_mut()
            .new_token(ast::Kind::QuestionToken);
        let colon = self.runtime.factory_mut().new_token(ast::Kind::ColonToken);
        self.runtime.factory_mut().new_conditional_expression(
            type_check,
            question,
            default_value,
            colon,
            value,
        )
    }

    fn create_destructuring_property_access(
        &mut self,
        value: ast::Node,
        property_name: ast::Node,
    ) -> ast::Node {
        if ast::is_computed_property_name(self.runtime.store_for(property_name), property_name) {
            let expression = self
                .runtime
                .store_for(property_name)
                .expression(property_name)
                .expect("computed property name should have expression");
            let property_name_loc = self.runtime.store_for(property_name).loc(property_name);
            let argument = self
                .runtime
                .visit_node(Some(expression))
                .expect("computed property name expression should not be removed");
            let argument = self.ensure_identifier(argument, false, property_name_loc);
            self.runtime.factory_mut().new_element_access_expression(
                value,
                None::<ast::Node>,
                argument,
                ast::NodeFlags::NONE,
            )
        } else if ast::is_string_or_numeric_literal_like(
            self.runtime.store_for(property_name),
            property_name,
        ) || ast::is_big_int_literal(self.runtime.store_for(property_name), property_name)
        {
            let argument = self.runtime.preserve_node(property_name);
            self.runtime.factory_mut().new_element_access_expression(
                value,
                None::<ast::Node>,
                argument,
                ast::NodeFlags::NONE,
            )
        } else {
            let text = self
                .runtime
                .store_for(property_name)
                .text(property_name)
                .to_string();
            let name = self.runtime.factory_mut().new_identifier(&text);
            self.runtime.factory_mut().new_property_access_expression(
                value,
                None::<ast::Node>,
                name,
                ast::NodeFlags::NONE,
            )
        }
    }

    fn emit_binding(
        &mut self,
        target: ast::Node,
        mut value: ast::Node,
        location: core::TextRange,
        original: Option<ast::Node>,
    ) {
        if !self.expressions.is_empty() {
            let mut expressions = std::mem::take(&mut self.expressions);
            expressions.push(value);
            value = self
                .runtime
                .emit_context
                .factory
                .inline_expressions(&expressions)
                .expect("inline expression receiver should not be empty");
        }
        self.declarations.push(PendingDecl {
            pending_expressions: Vec::new(),
            name: target,
            value,
            location,
            original,
        });
    }

    fn get_target_of_binding_or_assignment_element(&self, element: ast::Node) -> Option<ast::Node> {
        let source = self.runtime.store_for(element);
        match source.kind(element) {
            ast::Kind::VariableDeclaration | ast::Kind::Parameter | ast::Kind::BindingElement => {
                source.name(element)
            }
            ast::Kind::PropertyAssignment => source.initializer(element),
            ast::Kind::ShorthandPropertyAssignment | ast::Kind::SpreadAssignment => {
                source.name(element)
            }
            ast::Kind::BinaryExpression if ast::is_assignment_expression(source, element, true) => {
                source.left(element)
            }
            ast::Kind::SpreadElement => source.expression(element),
            _ => Some(element),
        }
    }

    fn get_initializer_of_binding_or_assignment_element(
        &self,
        element: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.runtime.store_for(element);
        match source.kind(element) {
            ast::Kind::VariableDeclaration | ast::Kind::Parameter | ast::Kind::BindingElement => {
                source.initializer(element)
            }
            ast::Kind::PropertyAssignment => {
                let initializer = source.initializer(element)?;
                if ast::is_assignment_expression(source, initializer, true) {
                    source.right(initializer)
                } else {
                    None
                }
            }
            ast::Kind::ShorthandPropertyAssignment => source.object_assignment_initializer(element),
            ast::Kind::BinaryExpression if ast::is_assignment_expression(source, element, true) => {
                source.right(element)
            }
            ast::Kind::SpreadElement => self
                .runtime
                .store_for(element)
                .expression(element)
                .and_then(|expr| self.get_initializer_of_binding_or_assignment_element(expr)),
            _ => None,
        }
    }

    fn is_declaration_binding_element(&self, element: ast::Node) -> bool {
        matches!(
            self.runtime.store_for(element).kind(element),
            ast::Kind::VariableDeclaration | ast::Kind::Parameter | ast::Kind::BindingElement
        )
    }

    fn element_contains_object_rest_or_spread(&self, element: ast::Node) -> bool {
        self.runtime
            .store_for(element)
            .subtree_facts(element)
            .intersects(ast::SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD)
    }

    fn element_contains_rest_or_spread(&self, element: ast::Node) -> bool {
        self.runtime
            .store_for(element)
            .subtree_facts(element)
            .intersects(
                ast::SubtreeFacts::CONTAINS_REST_OR_SPREAD
                    | ast::SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD,
            )
    }

    fn is_object_binding_or_assignment_pattern(&self, node: ast::Node) -> bool {
        matches!(
            self.runtime.store_for(node).kind(node),
            ast::Kind::ObjectBindingPattern | ast::Kind::ObjectLiteralExpression
        )
    }

    fn is_array_binding_or_assignment_pattern(&self, node: ast::Node) -> bool {
        matches!(
            self.runtime.store_for(node).kind(node),
            ast::Kind::ArrayBindingPattern | ast::Kind::ArrayLiteralExpression
        )
    }

    fn is_binding_or_assignment_pattern(&self, node: ast::Node) -> bool {
        self.is_object_binding_or_assignment_pattern(node)
            || self.is_array_binding_or_assignment_pattern(node)
    }

    fn is_simple_binding_or_assignment_element(&self, element: ast::Node) -> bool {
        let Some(target) = self.get_target_of_binding_or_assignment_element(element) else {
            return true;
        };
        if ast::is_omitted_expression(self.runtime.store_for(target), target) {
            return true;
        }
        let property_name = ast::try_get_property_name_of_binding_or_assignment_element(
            self.runtime.store_for(element),
            element,
        );
        if property_name
            .is_some_and(|name| !ast::is_property_name_literal(self.runtime.store_for(name), name))
        {
            return false;
        }
        if self
            .get_initializer_of_binding_or_assignment_element(element)
            .is_some_and(|initializer| !self.is_simple_inlineable_expression(initializer))
        {
            return false;
        }
        let target_source = self.runtime.store_for(target);
        if ast::is_binding_pattern(target_source, target) {
            return self
                .runtime
                .store_for(target)
                .source_elements(target)
                .is_none_or(|elements| {
                    elements
                        .iter()
                        .all(|element| self.is_simple_binding_or_assignment_element(element))
                });
        }
        ast::is_identifier(target_source, target)
    }

    fn is_simple_inlineable_expression(&self, node: ast::Node) -> bool {
        let source = self.runtime.store_for(node);
        !ast::is_identifier(source, node)
            && crate::utilities::is_simple_copiable_expression(source, &node)
    }
}

struct ObjectRestSpreadRuntime<'ctx, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'ctx mut printer::EmitContext,
    import_state: ast::AstImportState,
    compiler_options: &'ctx core::CompilerOptions,
    in_exported_variable_statement: bool,
    expression_result_is_unused: bool,
    parameters_with_preceding_object_rest_or_spread: Option<HashSet<ast::Node>>,
}

impl ObjectRestSpreadRuntime<'_, '_> {
    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        self.emit_context.store_for_node(node)
    }

    fn new_generated_name_for_node(&mut self, node: ast::Node) -> ast::Node {
        let original = self.emit_context.most_original(&node);
        if original.store_id() == self.source.store_id() {
            return self
                .emit_context
                .factory
                .new_generated_name_for_node(self.source, &original);
        }
        if let Some(source_file) = self.emit_context.source_file_handle_for_node(original) {
            return self
                .emit_context
                .factory
                .new_generated_name_for_node(source_file.store(), &original);
        }
        if node.store_id() == self.source.store_id() {
            return self
                .emit_context
                .factory
                .new_generated_name_for_node(self.source, &node);
        }
        self.emit_context.new_generated_name_for_node(node)
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

    fn update_parameter_declaration(
        &mut self,
        node: ast::Node,
        dot_dot_dot: Option<ast::Node>,
        name: Option<ast::Node>,
        initializer: Option<ast::Node>,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_parameter_declaration(
                node,
                None::<ast::ModifierList>,
                dot_dot_dot,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_parameter_declaration_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                dot_dot_dot,
                name,
                None::<ast::Node>,
                None::<ast::Node>,
                initializer,
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
        if node.store_id() == self.factory().store().store_id() {
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

    fn update_get_accessor_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
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
        if node.store_id() == self.factory().store().store_id() {
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

    fn update_method_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        asterisk: Option<ast::Node>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
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

    fn update_function_declaration(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        asterisk: Option<ast::Node>,
        name: Option<ast::Node>,
        parameters: ast::NodeList,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
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
        if node.store_id() == self.factory().store().store_id() {
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

    fn update_arrow_function(
        &mut self,
        node: ast::Node,
        modifiers: Option<ast::ModifierList>,
        parameters: ast::NodeList,
        equals: Option<ast::Node>,
        body: Option<ast::Node>,
    ) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
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

    fn visit(&mut self, node: &ast::Node) -> Option<ast::Node> {
        if !self.contains_es_object_rest_or_spread(*node)
            && self
                .parameters_with_preceding_object_rest_or_spread
                .is_none()
        {
            return Some(*node);
        }

        let expression_result_is_unused = self.expression_result_is_unused;
        self.expression_result_is_unused = false;
        let store = self.store_for(*node);
        let result = match store.kind(*node) {
            ast::Kind::SourceFile => Some(self.generated_visit_each_child(node)),
            ast::Kind::ObjectLiteralExpression => Some(self.visit_object_literal_expression(*node)),
            ast::Kind::BinaryExpression => {
                Some(self.visit_binary_expression(*node, expression_result_is_unused))
            }
            ast::Kind::ExpressionStatement => {
                self.expression_result_is_unused = true;
                Some(self.visit_expression_statement(*node))
            }
            ast::Kind::ParenthesizedExpression => {
                self.expression_result_is_unused = expression_result_is_unused;
                Some(self.generated_visit_each_child(node))
            }
            ast::Kind::ForOfStatement => Some(self.visit_for_of_statement(*node)),
            ast::Kind::VariableStatement => Some(self.visit_variable_statement(*node)),
            ast::Kind::VariableDeclarationList => Some(self.visit_variable_declaration_list(*node)),
            ast::Kind::VariableDeclaration => Some(self.visit_variable_declaration(*node)),
            ast::Kind::CatchClause => Some(self.visit_catch_clause(*node)),
            ast::Kind::Parameter => Some(self.visit_parameter(*node)),
            ast::Kind::Constructor
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::MethodDeclaration
            | ast::Kind::FunctionDeclaration
            | ast::Kind::ArrowFunction
            | ast::Kind::FunctionExpression => Some(self.visit_function_like(*node)),
            _ => Some(self.generated_visit_each_child(node)),
        };
        self.expression_result_is_unused = expression_result_is_unused;
        result
    }

    fn contains_es_object_rest_or_spread(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        if source.subtree_facts(node).intersects(
            ast::SubtreeFacts::CONTAINS_ES_OBJECT_REST_OR_SPREAD
                | ast::SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD,
        ) {
            return true;
        }
        if self.node_is_object_rest_or_spread(node) {
            return true;
        }
        let mut found = false;
        let _ = source.for_each_present_child(node, |child| {
            if self.contains_es_object_rest_or_spread(child) {
                found = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        found
    }

    fn contains_object_rest_or_spread(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        ast::contains_object_rest_or_spread(source, node)
    }

    fn contains_rest_or_spread(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        if source.subtree_facts(node).intersects(
            ast::SubtreeFacts::CONTAINS_REST_OR_SPREAD
                | ast::SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD,
        ) {
            return true;
        }
        if self.node_is_rest_or_spread(node) {
            return true;
        }
        let mut found = false;
        let _ = source.for_each_present_child(node, |child| {
            if self.contains_rest_or_spread(child) {
                found = true;
                std::ops::ControlFlow::Break(())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        found
    }

    fn node_is_rest_or_spread(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        match source.kind(node) {
            ast::Kind::SpreadAssignment | ast::Kind::SpreadElement => true,
            ast::Kind::BindingElement => source.dot_dot_dot_token(node).is_some(),
            _ => false,
        }
    }

    fn node_is_object_rest_or_spread(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        match source.kind(node) {
            ast::Kind::SpreadAssignment => true,
            ast::Kind::BindingElement => {
                source.dot_dot_dot_token(node).is_some()
                    && source.parent(node).is_some_and(|parent| {
                        source.kind(parent) == ast::Kind::ObjectBindingPattern
                    })
            }
            _ => false,
        }
    }

    fn visit_parameter(&mut self, node: ast::Node) -> ast::Node {
        if self
            .parameters_with_preceding_object_rest_or_spread
            .as_ref()
            .is_some_and(|parameters| parameters.contains(&node))
        {
            let (mut name, dot_dot_dot) = {
                let source = self.store_for(node);
                (source.name(node), source.dot_dot_dot_token(node))
            };
            if name.is_some_and(|name| ast::is_binding_pattern(self.store_for(name), name)) {
                name = Some(self.new_generated_name_for_node(node));
            } else {
                name = name.map(|name| self.preserve_node(name));
            }
            let dot_dot_dot = dot_dot_dot.map(|token| self.preserve_node(token));
            return self.update_parameter_declaration(node, dot_dot_dot, name, None);
        }

        if self.contains_object_rest_or_spread(node) {
            let (dot_dot_dot, initializer) = {
                let source = self.store_for(node);
                (source.dot_dot_dot_token(node), source.initializer(node))
            };
            let dot_dot_dot = dot_dot_dot.map(|token| self.preserve_node(token));
            let name = self.new_generated_name_for_node(node);
            let initializer =
                initializer.and_then(|initializer| self.visit_node(Some(initializer)));
            return self.update_parameter_declaration(node, dot_dot_dot, Some(name), initializer);
        }

        self.generated_visit_each_child(&node)
    }

    fn collect_parameters_with_preceding_object_rest_or_spread(
        &self,
        node: ast::Node,
    ) -> Option<HashSet<ast::Node>> {
        let parameters = self.store_for(node).source_parameters(node)?;
        let mut result = None::<HashSet<ast::Node>>;
        for parameter in parameters.iter() {
            if let Some(parameters) = result.as_mut() {
                parameters.insert(parameter);
            } else if self.contains_object_rest_or_spread(parameter) {
                result = Some(HashSet::new());
            }
        }
        result
    }

    fn with_parameter_list_context<R>(
        &mut self,
        node: ast::Node,
        cb: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let old = self.parameters_with_preceding_object_rest_or_spread.take();
        self.parameters_with_preceding_object_rest_or_spread =
            self.collect_parameters_with_preceding_object_rest_or_spread(node);
        let result = cb(self);
        self.parameters_with_preceding_object_rest_or_spread = old;
        result
    }

    fn visit_function_like(&mut self, node: ast::Node) -> ast::Node {
        self.with_parameter_list_context(node, |this| match this.store_for(node).kind(node) {
            ast::Kind::ArrowFunction => this.visit_arrow_function(node),
            ast::Kind::FunctionDeclaration => this.visit_function_declaration(node),
            ast::Kind::FunctionExpression => this.visit_function_expression(node),
            ast::Kind::MethodDeclaration => this.visit_method_declaration(node),
            ast::Kind::Constructor => this.visit_constructor_declaration(node),
            ast::Kind::GetAccessor => this.visit_get_accessor_declaration(node),
            ast::Kind::SetAccessor => this.visit_set_accessor_declaration(node),
            _ => this.generated_visit_each_child(&node),
        })
    }

    fn transform_function_body(&mut self, node: ast::Node) -> Option<ast::Node> {
        self.emit_context.start_variable_environment();
        let body = self
            .store_for(node)
            .body(node)
            .and_then(|body| self.visit_node(Some(body)));
        let mut extras = self.emit_context.end_variable_environment();
        self.emit_context.start_variable_environment();
        let new_statements = self.collect_object_rest_assignments(node);
        let object_rest_environment = self.emit_context.end_variable_environment();
        extras.extend(object_rest_environment);
        if new_statements.is_empty() && extras.is_empty() {
            return body;
        }

        let (body, suffix, body_loc, statements_loc, multi_line) = match body {
            Some(body) if ast::is_block(self.store_for(body), body) => {
                let source = self.store_for(body);
                let mut prefix = Vec::new();
                let mut suffix = Vec::new();
                let mut custom = false;
                let source_statements = source
                    .source_statements(body)
                    .expect("block should have statement list");
                let view = source_statements;
                let loc = view.loc();
                let range = view.range();
                let multi_line = source.multi_line(body).unwrap_or(true);
                let body_store_id = source.store_id();
                let statements_vec = view.iter().collect::<Vec<_>>();
                for statement in statements_vec {
                    if !custom && ast::is_prologue_directive(self.store_for(statement), statement) {
                        prefix.push(self.preserve_node(statement));
                    } else if self.emit_context.emit_flags(&statement) & printer::EF_CUSTOM_PROLOGUE
                        != 0
                    {
                        custom = true;
                        prefix.push(self.preserve_node(statement));
                    } else {
                        suffix.push(self.preserve_node(statement));
                    }
                }
                let mut output_statements = prefix;
                output_statements.extend(extras);
                output_statements.extend(new_statements);
                output_statements.extend(suffix);
                let list = self
                    .factory_mut()
                    .new_node_list(loc, range, output_statements);
                if body_store_id == self.factory().store().store_id() {
                    return Some(self.factory_mut().update_block(body, list, multi_line));
                }
                let source = self.source;
                return Some(
                    self.factory_mut()
                        .update_block_from_store(source, body, list, multi_line),
                );
            }
            Some(body) => {
                let ret = self.factory_mut().new_return_statement(Some(body));
                let loc = self.store_for(body).loc(body);
                self.factory_mut().place_emit_synthetic_node(ret, loc);
                let statements = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    loc,
                    Vec::<ast::Node>::new(),
                );
                let block = self.factory_mut().new_block(statements, true);
                (block, vec![ret], loc, loc, true)
            }
            None => {
                let loc = core::undefined_text_range();
                let statements =
                    self.factory_mut()
                        .new_node_list(loc, loc, Vec::<ast::Node>::new());
                let block = self.factory_mut().new_block(statements, true);
                (block, Vec::new(), loc, loc, true)
            }
        };

        let mut statements = Vec::new();
        statements.extend(extras);
        statements.extend(new_statements);
        statements.extend(suffix);
        let list = self
            .factory_mut()
            .new_node_list(statements_loc, statements_loc, statements);
        let block = self.factory_mut().update_block(body, list, multi_line);
        self.factory_mut()
            .place_emit_synthetic_node(block, body_loc);
        Some(block)
    }

    fn collect_object_rest_assignments(&mut self, node: ast::Node) -> Vec<ast::Node> {
        let mut contains_preceding_object_rest_or_spread = false;
        let mut results = Vec::new();
        let parameters = {
            let Some(parameters) = self.store_for(node).source_parameters(node) else {
                return results;
            };
            parameters.iter().collect::<Vec<_>>()
        };
        for parameter in parameters {
            let (name, initializer, parameter_loc) = {
                let parameter_source = self.store_for(parameter);
                (
                    parameter_source.name(parameter),
                    parameter_source.initializer(parameter),
                    parameter_source.loc(parameter),
                )
            };
            let Some(name) = name else {
                continue;
            };
            if contains_preceding_object_rest_or_spread {
                if ast::is_binding_pattern(self.store_for(name), name) {
                    if self
                        .store_for(name)
                        .source_elements(name)
                        .is_some_and(|elements| elements.iter().next().is_some())
                    {
                        let generated = self.new_generated_name_for_node(parameter);
                        if let Some(declarations) = self.flatten_destructuring_binding(
                            parameter,
                            Some(generated),
                            FlattenLevel::All,
                            false,
                            false,
                        ) {
                            results.push(
                                self.create_variable_statement_from_declarations(declarations),
                            );
                        }
                    } else if initializer.is_some() {
                        let name = self.new_generated_name_for_node(parameter);
                        let initializer = initializer
                            .and_then(|initializer| self.visit_node(Some(initializer)))
                            .expect("parameter initializer should visit to an expression");
                        let assignment = self
                            .emit_context
                            .factory
                            .new_assignment_expression(name, initializer);
                        let statement = self
                            .factory_mut()
                            .new_expression_statement(Some(assignment));
                        self.emit_context
                            .set_emit_flags(&statement, printer::EF_CUSTOM_PROLOGUE);
                        results.push(statement);
                    }
                } else if let Some(initializer) = initializer {
                    // Converts a parameter initializer into a function body statement, i.e.:
                    //
                    //  function f(x = 1) { }
                    //
                    // becomes
                    //
                    //  function f(x) {
                    //    if (typeof x === "undefined") { x = 1; }
                    //  }
                    let name = self.clone_node_preserve_location(name);
                    self.emit_context
                        .set_emit_flags(&name, printer::EF_NO_SOURCE_MAP);
                    let initializer = self
                        .visit_node(Some(initializer))
                        .expect("parameter initializer should visit to an expression");
                    self.emit_context.set_emit_flags(
                        &initializer,
                        printer::EF_NO_SOURCE_MAP | printer::EF_NO_COMMENTS,
                    );
                    let assignment = self
                        .emit_context
                        .factory
                        .new_assignment_expression(name, initializer);
                    self.factory_mut()
                        .place_emit_synthetic_node(assignment, parameter_loc);
                    self.emit_context
                        .set_emit_flags(&assignment, printer::EF_NO_COMMENTS);

                    let expression_statement = self
                        .factory_mut()
                        .new_expression_statement(Some(assignment));
                    let statements = self.factory_mut().new_node_list(
                        parameter_loc,
                        parameter_loc,
                        vec![expression_statement],
                    );
                    let block = self.factory_mut().new_block(statements, false);
                    self.factory_mut()
                        .place_emit_synthetic_node(block, parameter_loc);
                    self.emit_context.set_emit_flags(
                        &block,
                        printer::EF_SINGLE_LINE
                            | printer::EF_NO_TRAILING_SOURCE_MAP
                            | printer::EF_NO_TOKEN_SOURCE_MAPS
                            | printer::EF_NO_COMMENTS,
                    );

                    let name_check = self.clone_node_preserve_location(name);
                    self.emit_context
                        .set_emit_flags(&name_check, printer::EF_NO_SOURCE_MAP);
                    let type_check = self
                        .emit_context
                        .factory
                        .new_type_check(&name_check, "undefined");
                    let statement =
                        self.factory_mut()
                            .new_if_statement(type_check, block, None::<ast::Node>);
                    self.factory_mut()
                        .place_emit_synthetic_node(statement, parameter_loc);
                    self.emit_context.set_emit_flags(
                        &statement,
                        printer::EF_NO_TOKEN_SOURCE_MAPS
                            | printer::EF_NO_TRAILING_SOURCE_MAP
                            | printer::EF_CUSTOM_PROLOGUE
                            | printer::EF_NO_COMMENTS
                            | printer::EF_START_ON_NEW_LINE,
                    );
                    results.push(statement);
                }
            } else if self.contains_object_rest_or_spread(parameter) {
                contains_preceding_object_rest_or_spread = true;
                let generated = self.new_generated_name_for_node(parameter);
                if let Some(declarations) = self.flatten_destructuring_binding(
                    parameter,
                    Some(generated),
                    FlattenLevel::ObjectRest,
                    false,
                    true,
                ) {
                    results.push(self.create_variable_statement_from_declarations(declarations));
                }
            }
        }
        results
    }

    fn flatten_destructuring_binding(
        &mut self,
        node: ast::Node,
        rval: Option<ast::Node>,
        level: FlattenLevel,
        hoist_temp_variables: bool,
        skip_initializer: bool,
    ) -> Option<ast::Node> {
        BindingFlattener::new(self, level, hoist_temp_variables).flatten(
            node,
            rval,
            skip_initializer,
        )
    }

    fn create_variable_statement_from_declarations(
        &mut self,
        declarations: ast::Node,
    ) -> ast::Node {
        let store = self.store_for(declarations);
        let declarations = if store.kind(declarations) == ast::Kind::SyntaxList {
            let nodes = store
                .syntax_list_children(declarations)
                .expect("SyntaxList should have children")
                .iter()
                .flatten()
                .collect::<Vec<_>>();
            nodes
                .into_iter()
                .map(|node| self.preserve_node(node))
                .collect()
        } else {
            vec![self.preserve_node(declarations)]
        };
        let declarations = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            declarations,
        );
        let declaration_list = self
            .factory_mut()
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        let statement = self
            .factory_mut()
            .new_variable_statement(None::<ast::ModifierList>, declaration_list);
        self.emit_context
            .set_emit_flags(&statement, printer::EF_CUSTOM_PROLOGUE);
        statement
    }

    fn visit_expression_statement(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let expression = source
            .expression(node)
            .and_then(|expression| self.visit_node(Some(expression)));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_expression_statement(node, expression)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_expression_statement_from_store(source, node, expression)
        }
    }

    fn visit_variable_statement(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            let declaration_list = self
                .factory()
                .store()
                .declaration_list(node)
                .and_then(|declaration_list| self.visit_node(Some(declaration_list)));
            return self.factory_mut().update_variable_statement(
                node,
                None::<ast::ModifierList>,
                declaration_list,
            );
        }

        if ast::has_syntactic_modifier(self.store_for(node), node, ast::ModifierFlags::EXPORT) {
            let old = self.in_exported_variable_statement;
            self.in_exported_variable_statement = true;
            let result = self.generated_visit_each_child(&node);
            self.in_exported_variable_statement = old;
            result
        } else {
            self.generated_visit_each_child(&node)
        }
    }

    fn visit_variable_declaration_list(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() != self.factory().store().store_id() {
            return self.generated_visit_each_child(&node);
        }

        let (declarations, loc, range, has_trailing_comma, flags) = {
            let store = self.factory().store();
            let declarations = store
                .declarations(node)
                .expect("variable declaration list should have declarations");
            (
                declarations.nodes(),
                declarations.loc(),
                declarations.range(),
                declarations.has_trailing_comma(),
                store.flags(node),
            )
        };
        let mut visited = Vec::with_capacity(declarations.len());
        let mut changed = false;
        for declaration in declarations {
            let result = self.visit(&declaration);
            self.append_visited_node(declaration, result, &mut visited, &mut changed);
        }
        let declarations = self.factory_mut().new_node_list_with_trailing_comma(
            loc,
            range,
            visited,
            has_trailing_comma,
        );
        self.factory_mut()
            .update_variable_declaration_list(node, declarations, flags)
    }

    fn visit_variable_declaration(&mut self, node: ast::Node) -> ast::Node {
        if self.in_exported_variable_statement {
            self.in_exported_variable_statement = false;
            let result = self.visit_variable_declaration_worker(node, true);
            self.in_exported_variable_statement = true;
            return result;
        }
        self.visit_variable_declaration_worker(node, false)
    }

    fn visit_variable_declaration_worker(&mut self, node: ast::Node, exported: bool) -> ast::Node {
        let source = self.store_for(node);
        // If we are here it is because the name contains a binding pattern with a rest somewhere in it.
        if let Some(name) = source.name(node)
            && ast::is_binding_pattern(self.store_for(name), name)
            && self.contains_object_rest_or_spread(node)
        {
            if let Some(flattened) = self.flatten_destructuring_binding(
                node,
                None,
                FlattenLevel::ObjectRest,
                exported,
                false,
            ) {
                return flattened;
            }
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_catch_clause(&mut self, node: ast::Node) -> ast::Node {
        let source = self.store_for(node);
        let Some(variable_declaration) = source.variable_declaration(node) else {
            return self.generated_visit_each_child(&node);
        };
        let declaration_source = self.store_for(variable_declaration);
        let Some(name) = declaration_source.name(variable_declaration) else {
            return self.generated_visit_each_child(&node);
        };
        if ast::is_binding_pattern(self.store_for(name), name)
            && self.contains_object_rest_or_spread(name)
        {
            let generated = self.new_generated_name_for_node(name);
            let updated_decl =
                if variable_declaration.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_variable_declaration(
                        variable_declaration,
                        name,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        generated,
                    )
                } else {
                    assert_eq!(variable_declaration.store_id(), self.source.store_id());
                    let source = self.source;
                    self.factory_mut().update_variable_declaration_from_store(
                        source,
                        variable_declaration,
                        name,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        generated,
                    )
                };
            let visited_bindings = self.flatten_destructuring_binding(
                updated_decl,
                None,
                FlattenLevel::ObjectRest,
                false,
                false,
            );
            let mut block = self
                .store_for(node)
                .block(node)
                .and_then(|block| self.visit_node(Some(block)));
            if let (Some(visited_bindings), Some(block_node)) = (visited_bindings, block) {
                let store = self.store_for(visited_bindings);
                let declarations = if store.kind(visited_bindings) == ast::Kind::SyntaxList {
                    store
                        .syntax_list_children(visited_bindings)
                        .expect("SyntaxList should have children")
                        .iter()
                        .flatten()
                        .collect::<Vec<_>>()
                } else {
                    vec![visited_bindings]
                };
                let declarations = declarations
                    .into_iter()
                    .map(|node| self.preserve_node(node))
                    .collect::<Vec<_>>();
                let declarations = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    declarations,
                );
                let declaration_list = self
                    .factory_mut()
                    .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
                let new_statement = self
                    .factory_mut()
                    .new_variable_statement(None::<ast::ModifierList>, declaration_list);
                let block_store = self.store_for(block_node);
                let source_statements = block_store
                    .source_statements(block_node)
                    .expect("catch block should have statement list");
                let loc = source_statements.loc();
                let range = source_statements.range();
                let multi_line = block_store.multi_line(block_node).unwrap_or(true);
                let block_store_id = block_store.store_id();
                let existing = source_statements.iter().collect::<Vec<_>>();
                let mut statements = Vec::with_capacity(existing.len() + 1);
                statements.push(new_statement);
                for statement in existing {
                    statements.push(self.preserve_node(statement));
                }
                let statement_list = self.factory_mut().new_node_list(loc, range, statements);
                block = Some(if block_store_id == self.factory().store().store_id() {
                    self.factory_mut()
                        .update_block(block_node, statement_list, multi_line)
                } else {
                    assert_eq!(block_store_id, self.source.store_id());
                    let source = self.source;
                    self.factory_mut().update_block_from_store(
                        source,
                        block_node,
                        statement_list,
                        multi_line,
                    )
                });
            }
            let catch_declaration =
                if variable_declaration.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_variable_declaration(
                        variable_declaration,
                        generated,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        None::<ast::Node>,
                    )
                } else {
                    assert_eq!(variable_declaration.store_id(), self.source.store_id());
                    let source = self.source;
                    self.factory_mut().update_variable_declaration_from_store(
                        source,
                        variable_declaration,
                        generated,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        None::<ast::Node>,
                    )
                };
            return if node.store_id() == self.factory().store().store_id() {
                self.factory_mut()
                    .update_catch_clause(node, catch_declaration, block)
            } else {
                assert_eq!(node.store_id(), self.source.store_id());
                let source = self.source;
                self.factory_mut().update_catch_clause_from_store(
                    source,
                    node,
                    catch_declaration,
                    block,
                )
            };
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_for_of_statement(&mut self, node: ast::Node) -> ast::Node {
        let (initializer, statement, expression, await_modifier) = {
            let source = self.store_for(node);
            (
                source.initializer(node),
                source.statement(node),
                source.expression(node),
                source.await_modifier(node),
            )
        };
        let Some(initializer) = initializer else {
            return self.generated_visit_each_child(&node);
        };
        if self.contains_object_rest_or_spread(initializer)
            || (self.is_assignment_pattern(initializer)
                && self.contains_object_rest_or_spread(initializer))
        {
            let initializer_without_parens =
                ast::skip_parentheses(self.store_for(initializer), initializer);
            let is_variable_declaration_list = ast::is_variable_declaration_list(
                self.store_for(initializer_without_parens),
                initializer_without_parens,
            );
            if is_variable_declaration_list
                || self.is_assignment_pattern(initializer_without_parens)
            {
                let temp = self.emit_context.factory.new_temp_variable();
                let binding_statement = self.emit_context.factory.create_for_of_binding_statement(
                    self.source,
                    &initializer_without_parens,
                    &temp,
                );
                let mut statements = Vec::new();
                if let Some(binding_statement) = self.visit_node(Some(binding_statement)) {
                    statements.push(binding_statement);
                }

                let mut body_location = core::undefined_text_range();
                let mut statements_location = core::undefined_text_range();
                if let Some(statement) = statement {
                    if ast::is_block(self.store_for(statement), statement) {
                        let (source_statements, statement_loc, statement_list_loc) = {
                            let statement_store = self.store_for(statement);
                            let source_statements = statement_store
                                .source_statements(statement)
                                .expect("for-of block should have statements");
                            (
                                source_statements.iter().collect::<Vec<_>>(),
                                statement_store.loc(statement),
                                source_statements.loc(),
                            )
                        };
                        for statement in source_statements {
                            if let Some(visited) = self.visit_node(Some(statement)) {
                                statements.push(visited);
                            }
                        }
                        body_location = statement_loc;
                        statements_location = statement_list_loc;
                    } else {
                        let statement_loc = self.store_for(statement).loc(statement);
                        if let Some(visited) = self.visit_node(Some(statement)) {
                            statements.push(visited);
                            body_location = statement_loc;
                            statements_location = statement_loc;
                        }
                    }
                }

                let temp_declaration = self.factory_mut().new_variable_declaration(
                    temp,
                    None::<ast::Node>,
                    None::<ast::Node>,
                    None::<ast::Node>,
                );
                let initializer_loc = self.store_for(initializer).loc(initializer);
                let declarations = self.factory_mut().new_node_list(
                    core::undefined_text_range(),
                    initializer_loc,
                    vec![temp_declaration],
                );
                let list = self
                    .factory_mut()
                    .new_variable_declaration_list(declarations, ast::NodeFlags::LET);
                self.factory_mut()
                    .place_emit_synthetic_node(list, initializer_loc);

                let expression =
                    expression.and_then(|expression| self.visit_node(Some(expression)));
                let statements_list = self.factory_mut().new_node_list(
                    statements_location,
                    statements_location,
                    statements,
                );
                let block = self.factory_mut().new_block(statements_list, true);
                self.factory_mut()
                    .place_emit_synthetic_node(block, body_location);
                let await_modifier = await_modifier.map(|modifier| self.preserve_node(modifier));
                return if node.store_id() == self.factory().store().store_id() {
                    self.factory_mut().update_for_in_or_of_statement(
                        node,
                        await_modifier,
                        list,
                        expression,
                        block,
                    )
                } else {
                    let source = self.source;
                    self.factory_mut().update_for_in_or_of_statement_from_store(
                        source,
                        node,
                        await_modifier,
                        list,
                        expression,
                        block,
                    )
                };
            }
        }
        self.generated_visit_each_child(&node)
    }

    fn is_assignment_pattern(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        ast::is_array_literal_expression(source, node)
            || ast::is_object_literal_expression(source, node)
    }

    fn visit_binary_expression(
        &mut self,
        node: ast::Node,
        expression_result_is_unused: bool,
    ) -> ast::Node {
        let is_destructuring_assignment =
            ast::is_destructuring_assignment(self.store_for(node), node);
        let left_contains_object_rest_or_spread = self
            .store_for(node)
            .left(node)
            .is_some_and(|left| self.contains_object_rest_or_spread(left));
        if is_destructuring_assignment && left_contains_object_rest_or_spread {
            let flattened = crate::destructuring::flatten_destructuring_assignment(
                self.source,
                self.emit_context,
                node,
                !expression_result_is_unused,
                crate::destructuring::FlattenLevel::ObjectRest,
                None,
            );
            return self.visit_generated_expression(flattened);
        }
        let is_comma_expression =
            self.store_for(node)
                .operator_token(node)
                .is_some_and(|operator| {
                    self.store_for(operator).kind(operator) == ast::Kind::CommaToken
                });
        if is_comma_expression {
            self.expression_result_is_unused = true;
            let left = self
                .store_for(node)
                .left(node)
                .and_then(|left| self.visit_node(Some(left)));
            self.expression_result_is_unused = expression_result_is_unused;
            let right = self
                .store_for(node)
                .right(node)
                .and_then(|right| self.visit_node(Some(right)));
            let operator = self
                .store_for(node)
                .operator_token(node)
                .map(|operator| self.preserve_node(operator));
            if node.store_id() == self.factory().store().store_id() {
                return self.factory_mut().update_binary_expression(
                    node,
                    None::<ast::ModifierList>,
                    left,
                    None::<ast::Node>,
                    operator,
                    right,
                );
            } else {
                let source = self.source;
                return self.factory_mut().update_binary_expression_from_store(
                    source,
                    node,
                    None::<ast::ModifierList>,
                    left,
                    None::<ast::Node>,
                    operator,
                    right,
                );
            }
        }
        self.generated_visit_each_child(&node)
    }

    fn visit_object_literal_expression(&mut self, node: ast::Node) -> ast::Node {
        if !self
            .store_for(node)
            .subtree_facts(node)
            .intersects(ast::SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD)
        {
            return self.generated_visit_each_child(&node);
        }
        let mut objects = self.chunk_object_literal_elements(node);
        if objects.is_empty() {
            return self.generated_visit_each_child(&node);
        }
        if self.store_for(objects[0]).kind(objects[0]) != ast::Kind::ObjectLiteralExpression {
            let empty = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<ast::Node>::new(),
            );
            let empty = self
                .factory_mut()
                .new_object_literal_expression(empty, false);
            objects.insert(0, empty);
        }
        let mut expression = objects[0];
        if objects.len() > 1 {
            for object in objects.into_iter().skip(1) {
                expression = self.emit_context.factory.new_assign_helper(
                    &[expression, object],
                    self.compiler_options.get_emit_script_target(),
                );
            }
            expression
        } else {
            self.emit_context.factory.new_assign_helper(
                &[expression],
                self.compiler_options.get_emit_script_target(),
            )
        }
    }

    fn chunk_object_literal_elements(&mut self, node: ast::Node) -> Vec<ast::Node> {
        let properties = {
            let source = self.store_for(node);
            let Some(properties) = source.source_properties(node) else {
                return Vec::new();
            };
            properties.iter().collect::<Vec<_>>()
        };
        let mut chunk = Vec::new();
        let mut objects = Vec::new();
        for property in properties {
            let (property_kind, expression, name, initializer) = {
                let property_source = self.store_for(property);
                (
                    property_source.kind(property),
                    property_source.expression(property),
                    property_source.name(property),
                    property_source.initializer(property),
                )
            };
            if property_kind == ast::Kind::SpreadAssignment {
                if !chunk.is_empty() {
                    let list = self.factory_mut().new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        std::mem::take(&mut chunk),
                    );
                    objects.push(
                        self.factory_mut()
                            .new_object_literal_expression(list, false),
                    );
                }
                if let Some(expression) =
                    expression.and_then(|expression| self.visit_node(Some(expression)))
                {
                    objects.push(expression);
                }
            } else {
                let element = if property_kind == ast::Kind::PropertyAssignment {
                    let name = name.map(|name| self.preserve_node(name));
                    let initializer =
                        initializer.and_then(|initializer| self.visit_node(Some(initializer)));
                    self.factory_mut().new_property_assignment(
                        None::<ast::ModifierList>,
                        name,
                        None::<ast::Node>,
                        None::<ast::Node>,
                        initializer,
                    )
                } else {
                    self.visit_node(Some(property))
                        .expect("object literal element should not be removed")
                };
                chunk.push(element);
            }
        }
        if !chunk.is_empty() {
            let list = self.factory_mut().new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                chunk,
            );
            objects.push(
                self.factory_mut()
                    .new_object_literal_expression(list, false),
            );
        }
        objects
    }

    fn visit_generated_expression(&mut self, node: ast::Node) -> ast::Node {
        let kind = self.store_for(node).kind(node);
        match kind {
            ast::Kind::BinaryExpression => self.visit_generated_binary_expression(node),
            ast::Kind::ConditionalExpression => self.visit_generated_conditional_expression(node),
            ast::Kind::ObjectLiteralExpression => {
                self.visit_generated_object_literal_expression(node)
            }
            ast::Kind::PropertyAssignment => self.visit_generated_property_assignment(node),
            ast::Kind::ShorthandPropertyAssignment => {
                self.visit_generated_shorthand_property_assignment(node)
            }
            _ => self.preserve_node(node),
        }
    }

    fn visit_generated_conditional_expression(&mut self, node: ast::Node) -> ast::Node {
        let (condition, question_token, when_true, colon_token, when_false) = {
            let source = self.store_for(node);
            (
                source.condition(node),
                source.question_token(node),
                source.when_true(node),
                source.colon_token(node),
                source.when_false(node),
            )
        };
        let condition = condition.map(|condition| self.visit_generated_expression(condition));
        let question_token =
            question_token.map(|question_token| self.preserve_node(question_token));
        let when_true = when_true.map(|when_true| self.visit_generated_expression(when_true));
        let colon_token = colon_token.map(|colon_token| self.preserve_node(colon_token));
        let when_false = when_false.map(|when_false| self.visit_generated_expression(when_false));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_conditional_expression(
                node,
                condition,
                question_token,
                when_true,
                colon_token,
                when_false,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_conditional_expression_from_store(
                source,
                node,
                condition,
                question_token,
                when_true,
                colon_token,
                when_false,
            )
        }
    }

    fn visit_generated_binary_expression(&mut self, node: ast::Node) -> ast::Node {
        let (left, operator, right) = {
            let source = self.store_for(node);
            (
                source.left(node),
                source.operator_token(node),
                source.right(node),
            )
        };
        let left = left.map(|left| self.visit_generated_expression(left));
        let operator = operator.map(|operator| self.preserve_node(operator));
        let right = right.map(|right| self.visit_generated_expression(right));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_binary_expression(
                node,
                None::<ast::ModifierList>,
                left,
                None::<ast::Node>,
                operator,
                right,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_binary_expression_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                left,
                None::<ast::Node>,
                operator,
                right,
            )
        }
    }

    fn visit_generated_object_literal_expression(&mut self, node: ast::Node) -> ast::Node {
        let (properties, loc, range, has_trailing_comma, multi_line, has_spread_assignment) = {
            let source = self.store_for(node);
            let Some(properties) = source.source_properties(node) else {
                return self.preserve_node(node);
            };
            let property_nodes = properties.iter().collect::<Vec<_>>();
            let has_spread_assignment = property_nodes
                .iter()
                .any(|property| source.kind(*property) == ast::Kind::SpreadAssignment);
            (
                property_nodes,
                properties.loc(),
                properties.range(),
                properties.has_trailing_comma(),
                source.multi_line(node).unwrap_or(false),
                has_spread_assignment,
            )
        };
        if has_spread_assignment {
            return self.visit_object_literal_expression(node);
        }
        let properties = properties
            .into_iter()
            .map(|property| self.visit_generated_expression(property))
            .collect::<Vec<_>>();
        let properties = self.factory_mut().new_node_list_with_trailing_comma(
            loc,
            range,
            properties,
            has_trailing_comma,
        );
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut()
                .update_object_literal_expression(node, properties, multi_line)
        } else {
            let source = self.source;
            self.factory_mut()
                .update_object_literal_expression_from_store(source, node, properties, multi_line)
        }
    }

    fn visit_generated_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        let (name, postfix_token, type_node, initializer) = {
            let source = self.store_for(node);
            (
                source.name(node),
                source.postfix_token(node),
                source.r#type(node),
                source.initializer(node),
            )
        };
        let name = name.map(|name| self.preserve_node(name));
        let postfix_token = postfix_token.map(|postfix_token| self.preserve_node(postfix_token));
        let type_node = type_node.map(|type_node| self.preserve_node(type_node));
        let initializer =
            initializer.map(|initializer| self.visit_generated_expression(initializer));
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_property_assignment(
                node,
                None::<ast::ModifierList>,
                name,
                postfix_token,
                type_node,
                initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut().update_property_assignment_from_store(
                source,
                node,
                None::<ast::ModifierList>,
                name,
                postfix_token,
                type_node,
                initializer,
            )
        }
    }

    fn visit_constructor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("constructor parameters should exist");
        let body = self.transform_function_body(node);
        let body = self.emit_context.finish_visit_function_body(body);
        self.update_constructor_declaration(node, modifiers, parameters, body)
    }

    fn visit_get_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let name = self
            .store_for(node)
            .name(node)
            .and_then(|name| self.visit_node(Some(name)));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("accessor parameters should exist");
        let body = self.transform_function_body(node);
        let body = self.emit_context.finish_visit_function_body(body);
        self.update_get_accessor_declaration(node, modifiers, name, parameters, body)
    }

    fn visit_set_accessor_declaration(&mut self, node: ast::Node) -> ast::Node {
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let name = self
            .store_for(node)
            .name(node)
            .and_then(|name| self.visit_node(Some(name)));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("accessor parameters should exist");
        let body = self.transform_function_body(node);
        let body = self.emit_context.finish_visit_function_body(body);
        self.update_set_accessor_declaration(node, modifiers, name, parameters, body)
    }

    fn visit_method_declaration(&mut self, node: ast::Node) -> ast::Node {
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let (asterisk, name) = {
            let source = self.store_for(node);
            (source.asterisk_token(node), source.name(node))
        };
        let asterisk = asterisk.map(|node| self.preserve_node(node));
        let name = name.and_then(|name| self.visit_node(Some(name)));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("method parameters should exist");
        let body = self.transform_function_body(node);
        let body = self.emit_context.finish_visit_function_body(body);
        self.update_method_declaration(node, modifiers, asterisk, name, parameters, body)
    }

    fn visit_function_declaration(&mut self, node: ast::Node) -> ast::Node {
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let (asterisk, name) = {
            let source = self.store_for(node);
            (source.asterisk_token(node), source.name(node))
        };
        let asterisk = asterisk.map(|node| self.preserve_node(node));
        let name = name.and_then(|name| self.visit_node(Some(name)));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("function parameters should exist");
        let body = self.transform_function_body(node);
        let body = self.emit_context.finish_visit_function_body(body);
        self.update_function_declaration(node, modifiers, asterisk, name, parameters, body)
    }

    fn visit_function_expression(&mut self, node: ast::Node) -> ast::Node {
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let (asterisk, name) = {
            let source = self.store_for(node);
            (source.asterisk_token(node), source.name(node))
        };
        let asterisk = asterisk.map(|node| self.preserve_node(node));
        let name = name.and_then(|name| self.visit_node(Some(name)));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("function parameters should exist");
        let body = self.transform_function_body(node);
        let body = self.emit_context.finish_visit_function_body(body);
        self.update_function_expression(node, modifiers, asterisk, name, parameters, body)
    }

    fn visit_arrow_function(&mut self, node: ast::Node) -> ast::Node {
        let modifiers = self.visit_modifiers_input(self.source_modifiers_input(node));
        let parameters = self
            .visit_parameters_input(self.source_parameters_input(node))
            .expect("arrow function parameters should exist");
        let equals = self.store_for(node).equals_greater_than_token(node);
        let equals = equals.map(|token| self.preserve_node(token));
        let body = self.transform_function_body(node);
        let body = self.emit_context.finish_visit_function_body(body);
        self.update_arrow_function(node, modifiers, parameters, equals, body)
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

    fn visit_generated_shorthand_property_assignment(&mut self, node: ast::Node) -> ast::Node {
        let (
            original_name,
            original_postfix_token,
            original_type_node,
            original_equals_token,
            original_object_assignment_initializer,
        ) = {
            let source = self.store_for(node);
            (
                source.name(node),
                source.postfix_token(node),
                source.r#type(node),
                source.equals_token(node),
                source.object_assignment_initializer(node),
            )
        };
        let name = original_name.map(|name| self.preserve_node(name));
        let postfix_token =
            original_postfix_token.map(|postfix_token| self.preserve_node(postfix_token));
        let type_node = original_type_node.map(|type_node| self.preserve_node(type_node));
        let equals_token =
            original_equals_token.map(|equals_token| self.preserve_node(equals_token));
        let object_assignment_initializer = original_object_assignment_initializer
            .map(|initializer| self.visit_generated_expression(initializer));
        if self.preserved_source_node_matches(original_name, name)
            && self.preserved_source_node_matches(original_postfix_token, postfix_token)
            && self.preserved_source_node_matches(original_type_node, type_node)
            && self.preserved_source_node_matches(original_equals_token, equals_token)
            && self.preserved_source_node_matches(
                original_object_assignment_initializer,
                object_assignment_initializer,
            )
        {
            return self.preserve_node(node);
        }
        if node.store_id() == self.factory().store().store_id() {
            self.factory_mut().update_shorthand_property_assignment(
                node,
                None::<ast::ModifierList>,
                name,
                postfix_token,
                type_node,
                equals_token,
                object_assignment_initializer,
            )
        } else {
            let source = self.source;
            self.factory_mut()
                .update_shorthand_property_assignment_from_store(
                    source,
                    node,
                    None::<ast::ModifierList>,
                    name,
                    postfix_token,
                    type_node,
                    equals_token,
                    object_assignment_initializer,
                )
        }
    }

    fn clone_node_preserve_location(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return self
                .factory_mut()
                .deep_clone_node_in_current_store_preserve_location(node);
        }
        let source = self.source;
        self.factory_mut()
            .deep_clone_node_from_store_preserve_location(source, node)
    }
}

impl<'source> ast::AstVisitEachChildRuntime<'source> for ObjectRestSpreadRuntime<'_, 'source> {
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

impl<'source> ast::AstGeneratedVisitEachChild<'source> for ObjectRestSpreadRuntime<'_, 'source> {}
