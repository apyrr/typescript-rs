use ts_ast as ast;
use ts_core as core;
use ts_printer as printer;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum FlattenLevel {
    All,
    ObjectRest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlattenMode {
    Assignment,
    Binding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternKind {
    Object,
    Array,
    Other,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DestructuringFacts {
    pub is_destructuring_assignment: bool,
    pub needs_value: bool,
    pub value_is_identifier: bool,
    pub assigns_to_value_name: bool,
    pub contains_non_literal_computed_name: bool,
    pub node_is_synthesized: bool,
    pub has_initializer: bool,
    pub initializer_is_simple_inlineable: bool,
    pub target_is_pattern: bool,
    pub target_is_identifier: bool,
    pub target_is_omitted_expression: bool,
    pub property_name_is_literal: bool,
    pub property_name_is_computed: bool,
    pub pattern_element_count: usize,
    pub all_elements_omitted: bool,
    pub element_contains_object_rest_or_spread: bool,
    pub prior_element_was_transformed: bool,
}

pub fn flatten_assignment_hoists_temp_variables() -> bool {
    true
}

pub fn assignment_value_needs_identifier(facts: DestructuringFacts) -> bool {
    (facts.value_is_identifier && facts.assigns_to_value_name)
        || facts.contains_non_literal_computed_name
        || facts.needs_value
}

pub fn binding_initializer_needs_identifier(facts: DestructuringFacts) -> bool {
    facts.value_is_identifier
        && (facts.assigns_to_value_name || facts.contains_non_literal_computed_name)
}

pub fn default_value_check_needs_temp(facts: DestructuringFacts) -> bool {
    facts.has_initializer && !facts.initializer_is_simple_inlineable && facts.target_is_pattern
}

pub fn pattern_kind(is_object_pattern: bool, is_array_pattern: bool) -> PatternKind {
    if is_object_pattern {
        PatternKind::Object
    } else if is_array_pattern {
        PatternKind::Array
    } else {
        PatternKind::Other
    }
}

pub fn object_element_can_remain_grouped(level: FlattenLevel, facts: DestructuringFacts) -> bool {
    level >= FlattenLevel::ObjectRest
        && !facts.element_contains_object_rest_or_spread
        && !facts.property_name_is_computed
}

pub fn object_pattern_value_needs_temp(
    pattern_element_count: usize,
    is_declaration_binding_element: bool,
) -> bool {
    pattern_element_count != 1 && (!is_declaration_binding_element || pattern_element_count != 0)
}

pub fn array_pattern_value_needs_temp(level: FlattenLevel, facts: DestructuringFacts) -> bool {
    (facts.pattern_element_count != 1
        && (level < FlattenLevel::ObjectRest || facts.pattern_element_count == 0))
        || facts.all_elements_omitted
}

pub fn array_element_needs_temp_at_object_rest_level(facts: DestructuringFacts) -> bool {
    facts.element_contains_object_rest_or_spread
        || (facts.prior_element_was_transformed && !is_simple_binding_or_assignment_element(facts))
}

pub fn is_simple_binding_or_assignment_element(facts: DestructuringFacts) -> bool {
    (facts.target_is_omitted_expression || facts.target_is_identifier)
        && facts.property_name_is_literal
        && (!facts.has_initializer || facts.initializer_is_simple_inlineable)
}

// CreateAssignmentCallback is a callback used to create custom assignment expressions during destructuring flattening.
// When provided, the target will always be an Identifier, and the callback can wrap the assignment with additional logic
// (e.g., export expressions in CJS modules or namespace member assignments).
pub type CreateAssignmentCallback<'a> = &'a mut dyn FnMut(
    &mut printer::EmitContext,
    ast::Node,
    ast::Node,
    core::TextRange,
) -> ast::Node;

// FlattenDestructuringAssignment flattens a destructuring assignment expression into a sequence of
// individual property/element access assignments. Supports custom assignment callbacks for module
// export or namespace member expressions.
pub fn flatten_destructuring_assignment<'a>(
    source: &ast::AstStore,
    emit_context: &'a mut printer::EmitContext,
    node: ast::Node, // VariableDeclaration | DestructuringAssignment
    needs_value: bool,
    level: FlattenLevel,
    create_assignment_callback: Option<CreateAssignmentCallback<'a>>,
) -> ast::Node {
    let mut f = AssignmentFlattener::new(source, emit_context, level);
    f.create_assignment_callback = create_assignment_callback;
    f.hoist_temp_variables = true;
    f.flatten_destructuring_assignment(node, needs_value)
}

struct AssignmentFlattener<'a, 'source> {
    source: &'source ast::AstStore,
    emit_context: &'a mut printer::EmitContext,
    import_state: ast::AstImportState,
    level: FlattenLevel,
    create_assignment_callback: Option<CreateAssignmentCallback<'a>>,
    expressions: Vec<ast::Node>,
    has_transformed_prior_element: bool,
    hoist_temp_variables: bool,
}

impl<'a, 'source> AssignmentFlattener<'a, 'source> {
    fn new(
        source: &'source ast::AstStore,
        emit_context: &'a mut printer::EmitContext,
        level: FlattenLevel,
    ) -> Self {
        Self {
            source,
            emit_context,
            import_state: ast::AstImportState::new(),
            level,
            create_assignment_callback: None,
            expressions: Vec::new(),
            has_transformed_prior_element: false,
            hoist_temp_variables: false,
        }
    }

    fn factory(&self) -> &ast::NodeFactory {
        &self.emit_context.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.emit_context.factory.node_factory
    }

    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        ast::AstImportState::store_for(self.source, self.factory(), node)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.factory().store().store_id() {
            return node;
        }
        let mut import_state = std::mem::take(&mut self.import_state);
        let imported = import_state.preserve_node(self.source, self.factory_mut(), node);
        self.import_state = import_state;
        imported
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.preserve_node(node))
    }

    fn emit_expression(&mut self, expression: ast::Node) {
        self.expressions.push(expression);
    }

    fn ensure_identifier(
        &mut self,
        value: ast::Node,
        reuse_identifier_expressions: bool,
        location: core::TextRange,
    ) -> ast::Node {
        let store = self.store_for(value);
        if reuse_identifier_expressions && ast::is_identifier(store, value) {
            return value;
        }
        let temp = self.emit_context.factory.new_temp_variable();
        if self.hoist_temp_variables {
            self.emit_context.add_variable_declaration(temp);
            let assign = self
                .emit_context
                .factory
                .new_assignment_expression(temp, value);
            self.factory_mut()
                .place_emit_synthetic_node(assign, location);
            self.emit_expression(assign);
        } else {
            self.emit_assignment(temp, value, location, None);
        }
        temp
    }

    fn create_default_value_check(
        &mut self,
        value: ast::Node,
        default_value: ast::Node,
        location: core::TextRange,
    ) -> ast::Node {
        let value = self.ensure_identifier(value, true, location);
        let type_check = self
            .emit_context
            .factory
            .new_type_check(&value, "undefined");
        let question = self.factory_mut().new_token(ast::Kind::QuestionToken);
        let colon = self.factory_mut().new_token(ast::Kind::ColonToken);
        self.factory_mut().new_conditional_expression(
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
        let source = self.store_for(property_name);
        if ast::is_computed_property_name(source, property_name) {
            let expression = source
                .expression(property_name)
                .expect("computed property name should have expression");
            let argument = self
                .visit_node(Some(expression))
                .expect("computed property name expression should not be removed");
            let source = self.store_for(property_name);
            let argument = self.ensure_identifier(argument, false, source.loc(property_name));
            self.factory_mut().new_element_access_expression(
                value,
                None::<ast::Node>,
                argument,
                ast::NodeFlags::NONE,
            )
        } else if ast::is_string_or_numeric_literal_like(source, property_name)
            || ast::is_big_int_literal(source, property_name)
        {
            let argument = self.preserve_node(property_name);
            self.factory_mut().new_element_access_expression(
                value,
                None::<ast::Node>,
                argument,
                ast::NodeFlags::NONE,
            )
        } else {
            let text = source.text(property_name);
            let name = self.factory_mut().new_identifier(&text);
            self.factory_mut().new_property_access_expression(
                value,
                None::<ast::Node>,
                name,
                ast::NodeFlags::NONE,
            )
        }
    }

    fn flatten_destructuring_assignment(
        mut self,
        mut node: ast::Node,
        needs_value: bool,
    ) -> ast::Node {
        let mut location = self.store_for(node).loc(node);
        let mut value = None;
        if ast::is_destructuring_assignment(self.store_for(node), node) {
            value = self.store_for(node).right(node);
            while self.store_for(node).left(node).is_some_and(|left| {
                self.is_empty_array_literal(left) || self.is_empty_object_literal(left)
            }) {
                if let Some(next) = value
                    && ast::is_destructuring_assignment(self.store_for(next), next)
                {
                    node = next;
                    location = self.store_for(node).loc(node);
                    value = self.store_for(node).right(node);
                } else {
                    return self
                        .visit_node(value)
                        .unwrap_or_else(|| self.emit_context.factory.new_void_zero_expression());
                }
            }
        }

        let mut value = if let Some(value) = value {
            let mut value = self
                .visit_node(Some(value))
                .expect("destructuring assignment value should not be removed");
            let value_store = self.store_for(value);
            if (ast::is_identifier(value_store, value)
                && binding_or_assignment_element_assigns_to_name(
                    self.store_for(node),
                    node,
                    &value_store.text(value),
                ))
                || binding_or_assignment_element_contains_non_literal_computed_name(
                    self.store_for(node),
                    node,
                )
            {
                value = self.ensure_identifier(value, false, location);
            } else if needs_value {
                value = self.ensure_identifier(value, true, location);
            } else if ast::node_is_synthesized(self.store_for(node), node) {
                location = self.store_for(value).loc(value);
            }
            Some(value)
        } else {
            None
        };

        self.flatten_binding_or_assignment_element(
            node,
            value,
            location,
            ast::is_destructuring_assignment(self.store_for(node), node),
        );

        if let Some(value) = value.take()
            && needs_value
        {
            if self.expressions.is_empty() {
                return value;
            }
            self.expressions.push(value);
        }

        self.emit_context
            .factory
            .inline_expressions(&self.expressions)
            .unwrap_or_else(|| self.factory_mut().new_omitted_expression())
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
            let initializer = self
                .get_initializer_of_binding_or_assignment_element(element)
                .and_then(|initializer| self.visit_node(Some(initializer)));
            if let Some(initializer) = initializer {
                if let Some(current_value) = value {
                    value =
                        Some(self.create_default_value_check(current_value, initializer, location));
                    if !self.is_simple_inlineable_expression(initializer)
                        && self.is_binding_or_assignment_pattern(binding_target)
                    {
                        value = value.map(|value| self.ensure_identifier(value, true, location));
                    }
                } else {
                    value = Some(initializer);
                }
            } else if value.is_none() {
                value = Some(self.emit_context.factory.new_void_zero_expression());
            }
        }

        let value = value.expect("destructuring assignment element should have a value");
        if self.is_object_binding_or_assignment_pattern(binding_target) {
            self.flatten_object_binding_or_assignment_pattern(
                element,
                binding_target,
                value,
                location,
            );
        } else if self.is_array_binding_or_assignment_pattern(binding_target) {
            self.flatten_array_binding_or_assignment_pattern(
                element,
                binding_target,
                value,
                location,
            );
        } else {
            self.emit_assignment(binding_target, value, location, Some(element));
        }
    }

    fn flatten_object_binding_or_assignment_pattern(
        &mut self,
        parent: ast::Node,
        pattern: ast::Node,
        mut value: ast::Node,
        location: core::TextRange,
    ) {
        let pattern_store = self.store_for(pattern);
        let elements_vec = get_elements_of_binding_or_assignment_pattern(pattern_store, pattern);
        if elements_vec.len() != 1 {
            let reuse_identifier_expressions =
                !self.is_declaration_binding_element(parent) || !elements_vec.is_empty();
            value = self.ensure_identifier(value, reuse_identifier_expressions, location);
        }
        let mut binding_elements = Vec::new();
        let mut computed_temp_variables = Vec::new();
        for (index, element) in elements_vec.iter().copied().enumerate() {
            if self
                .get_rest_indicator_of_binding_or_assignment_element(element)
                .is_none()
            {
                let property_name = ast::try_get_property_name_of_binding_or_assignment_element(
                    self.store_for(element),
                    element,
                );
                let can_remain_grouped = self.level >= FlattenLevel::ObjectRest
                    && !self.element_contains_rest_or_spread(element)
                    && self
                        .get_target_of_binding_or_assignment_element(element)
                        .is_some_and(|target| !self.element_contains_rest_or_spread(target))
                    && property_name.is_none_or(|name| {
                        !ast::is_computed_property_name(self.store_for(name), name)
                    });
                if can_remain_grouped {
                    if let Some(visited) = self.visit_node(Some(element)) {
                        binding_elements.push(visited);
                    }
                } else {
                    if !binding_elements.is_empty() {
                        let target = self.create_object_assignment_pattern(&mut binding_elements);
                        self.emit_assignment(target, value, location, Some(pattern));
                    }
                    let Some(property_name) = property_name else {
                        continue;
                    };
                    let rhs_value = self.create_destructuring_property_access(value, property_name);
                    if ast::is_computed_property_name(self.store_for(property_name), property_name)
                        && let Some(argument) =
                            self.store_for(rhs_value).argument_expression(rhs_value)
                    {
                        computed_temp_variables.push(argument);
                    }
                    self.flatten_binding_or_assignment_element(
                        element,
                        Some(rhs_value),
                        self.store_for(element).loc(element),
                        false,
                    );
                }
            } else if index == elements_vec.len() - 1 {
                if !binding_elements.is_empty() {
                    let target = self.create_object_assignment_pattern(&mut binding_elements);
                    self.emit_assignment(target, value, location, Some(pattern));
                }
                let pattern_loc = self.store_for(pattern).loc(pattern);
                let computed_temp_variables = (!computed_temp_variables.is_empty())
                    .then_some(computed_temp_variables.as_slice());
                let rhs_value = if pattern.store_id() == self.factory().store().store_id() {
                    self.emit_context.factory.new_rest_helper_current_store(
                        value,
                        &elements_vec,
                        computed_temp_variables,
                        pattern_loc,
                    )
                } else {
                    self.emit_context.factory.new_rest_helper(
                        self.source,
                        value,
                        &elements_vec,
                        computed_temp_variables,
                        pattern_loc,
                    )
                };
                self.flatten_binding_or_assignment_element(
                    element,
                    Some(rhs_value),
                    self.store_for(element).loc(element),
                    false,
                );
            }
        }
        if !binding_elements.is_empty() {
            let target = self.create_object_assignment_pattern(&mut binding_elements);
            self.emit_assignment(target, value, location, Some(pattern));
        }
    }

    fn flatten_array_binding_or_assignment_pattern(
        &mut self,
        parent: ast::Node,
        pattern: ast::Node,
        mut value: ast::Node,
        location: core::TextRange,
    ) {
        let pattern_store = self.store_for(pattern);
        let elements_vec = get_elements_of_binding_or_assignment_pattern(pattern_store, pattern);
        let all_omitted = elements_vec
            .iter()
            .all(|element| is_array_binding_elision(self.store_for(*element), *element));
        if (elements_vec.len() != 1
            && (self.level < FlattenLevel::ObjectRest || elements_vec.is_empty()))
            || all_omitted
        {
            let reuse_identifier_expressions =
                !self.is_declaration_binding_element(parent) || !elements_vec.is_empty();
            value = self.ensure_identifier(value, reuse_identifier_expressions, location);
        }
        let mut binding_elements = Vec::new();
        let mut rest_containing_elements = Vec::<(ast::Node, ast::Node)>::new();
        for (index, element) in elements_vec.iter().copied().enumerate() {
            if self.level >= FlattenLevel::ObjectRest {
                if self.element_contains_object_rest_or_spread(element)
                    || (self.has_transformed_prior_element
                        && !self.is_simple_binding_or_assignment_element(element))
                {
                    self.has_transformed_prior_element = true;
                    let temp = self.emit_context.factory.new_temp_variable();
                    if self.hoist_temp_variables {
                        self.emit_context.add_variable_declaration(temp);
                    }
                    rest_containing_elements.push((temp, element));
                    binding_elements.push(self.create_array_assignment_element(temp));
                } else if let Some(visited) = self.visit_node(Some(element)) {
                    binding_elements.push(visited);
                }
            } else if is_array_binding_elision(self.store_for(element), element) {
                continue;
            } else if self
                .get_rest_indicator_of_binding_or_assignment_element(element)
                .is_none()
            {
                let index = self
                    .factory_mut()
                    .new_numeric_literal(&index.to_string(), ast::TokenFlags::NONE);
                let rhs_value = self.factory_mut().new_element_access_expression(
                    value,
                    None::<ast::Node>,
                    index,
                    ast::NodeFlags::NONE,
                );
                self.flatten_binding_or_assignment_element(
                    element,
                    Some(rhs_value),
                    self.store_for(element).loc(element),
                    false,
                );
            } else if index == elements_vec.len() - 1 {
                let rhs_value = self
                    .emit_context
                    .factory
                    .new_array_slice_call(&value, index as i32);
                self.flatten_binding_or_assignment_element(
                    element,
                    Some(rhs_value),
                    self.store_for(element).loc(element),
                    false,
                );
            }
        }
        if !binding_elements.is_empty() {
            let target = self.create_array_assignment_pattern(&mut binding_elements);
            self.emit_assignment(target, value, location, Some(pattern));
        }
        for (temp, element) in rest_containing_elements {
            self.flatten_binding_or_assignment_element(
                element,
                Some(temp),
                self.store_for(element).loc(element),
                false,
            );
        }
    }

    fn create_array_assignment_pattern(&mut self, elements: &mut Vec<ast::Node>) -> ast::Node {
        let elements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            std::mem::take(elements),
        );
        self.factory_mut()
            .new_array_literal_expression(elements, false)
    }

    fn create_object_assignment_pattern(&mut self, elements: &mut Vec<ast::Node>) -> ast::Node {
        let elements = self.factory_mut().new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            std::mem::take(elements),
        );
        self.factory_mut()
            .new_object_literal_expression(elements, false)
    }

    fn create_array_assignment_element(&mut self, expression: ast::Node) -> ast::Node {
        expression
    }

    fn emit_assignment(
        &mut self,
        target: ast::Node,
        value: ast::Node,
        location: core::TextRange,
        original: Option<ast::Node>,
    ) {
        let target_is_identifier = ast::is_identifier(self.store_for(target), target);
        let expression = if target_is_identifier
            && let Some(callback) = self.create_assignment_callback.as_mut()
        {
            callback(self.emit_context, target, value, location)
        } else {
            let target = self
                .visit_node(Some(target))
                .expect("destructuring assignment target should not be removed");
            let expression = self
                .emit_context
                .factory
                .new_assignment_expression(target, value);
            self.factory_mut()
                .place_emit_synthetic_node(expression, location);
            expression
        };
        if let Some(original) = original {
            self.emit_context.set_original(&expression, &original);
        }
        self.emit_expression(expression);
    }

    fn get_target_of_binding_or_assignment_element(&self, element: ast::Node) -> Option<ast::Node> {
        let source = self.store_for(element);
        match source.kind(element) {
            ast::Kind::VariableDeclaration | ast::Kind::Parameter | ast::Kind::BindingElement => {
                source.name(element)
            }
            ast::Kind::PropertyAssignment => source.initializer(element).and_then(|initializer| {
                self.get_target_of_binding_or_assignment_element(initializer)
            }),
            ast::Kind::ShorthandPropertyAssignment => source.name(element),
            ast::Kind::SpreadAssignment => source.expression(element).and_then(|expression| {
                self.get_target_of_binding_or_assignment_element(expression)
            }),
            ast::Kind::BinaryExpression if ast::is_assignment_expression(source, element, true) => {
                source
                    .left(element)
                    .and_then(|left| self.get_target_of_binding_or_assignment_element(left))
            }
            ast::Kind::SpreadElement => source.expression(element).and_then(|expression| {
                self.get_target_of_binding_or_assignment_element(expression)
            }),
            _ => Some(element),
        }
    }

    fn get_initializer_of_binding_or_assignment_element(
        &self,
        element: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(element);
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
            ast::Kind::SpreadElement => source
                .expression(element)
                .and_then(|expr| self.get_initializer_of_binding_or_assignment_element(expr)),
            _ => None,
        }
    }

    fn get_rest_indicator_of_binding_or_assignment_element(
        &self,
        element: ast::Node,
    ) -> Option<ast::Node> {
        let source = self.store_for(element);
        match source.kind(element) {
            ast::Kind::BindingElement | ast::Kind::Parameter => source.dot_dot_dot_token(element),
            ast::Kind::SpreadAssignment | ast::Kind::SpreadElement => Some(element),
            _ => None,
        }
    }

    fn is_declaration_binding_element(&self, element: ast::Node) -> bool {
        matches!(
            self.store_for(element).kind(element),
            ast::Kind::VariableDeclaration | ast::Kind::Parameter | ast::Kind::BindingElement
        )
    }

    fn is_object_binding_or_assignment_pattern(&self, node: ast::Node) -> bool {
        matches!(
            self.store_for(node).kind(node),
            ast::Kind::ObjectBindingPattern | ast::Kind::ObjectLiteralExpression
        )
    }

    fn is_array_binding_or_assignment_pattern(&self, node: ast::Node) -> bool {
        matches!(
            self.store_for(node).kind(node),
            ast::Kind::ArrayBindingPattern | ast::Kind::ArrayLiteralExpression
        )
    }

    fn is_binding_or_assignment_pattern(&self, node: ast::Node) -> bool {
        self.is_object_binding_or_assignment_pattern(node)
            || self.is_array_binding_or_assignment_pattern(node)
    }

    fn is_empty_array_literal(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        ast::is_array_literal_expression(source, node)
            && get_elements_of_binding_or_assignment_pattern(source, node).is_empty()
    }

    fn is_empty_object_literal(&self, node: ast::Node) -> bool {
        let source = self.store_for(node);
        ast::is_object_literal_expression(source, node)
            && get_elements_of_binding_or_assignment_pattern(source, node).is_empty()
    }

    fn element_contains_rest_or_spread(&self, element: ast::Node) -> bool {
        self.store_for(element).subtree_facts(element).intersects(
            ast::SubtreeFacts::CONTAINS_REST_OR_SPREAD
                | ast::SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD,
        )
    }

    fn element_contains_object_rest_or_spread(&self, element: ast::Node) -> bool {
        self.store_for(element)
            .subtree_facts(element)
            .intersects(ast::SubtreeFacts::CONTAINS_OBJECT_REST_OR_SPREAD)
    }

    fn is_simple_binding_or_assignment_element(&self, element: ast::Node) -> bool {
        let Some(target) = self.get_target_of_binding_or_assignment_element(element) else {
            return true;
        };
        if ast::is_omitted_expression(self.store_for(target), target) {
            return true;
        }
        let property_name = ast::try_get_property_name_of_binding_or_assignment_element(
            self.store_for(element),
            element,
        );
        if property_name
            .is_some_and(|name| !ast::is_property_name_literal(self.store_for(name), name))
        {
            return false;
        }
        if self
            .get_initializer_of_binding_or_assignment_element(element)
            .is_some_and(|initializer| !self.is_simple_inlineable_expression(initializer))
        {
            return false;
        }
        if self.is_binding_or_assignment_pattern(target) {
            return get_elements_of_binding_or_assignment_pattern(self.store_for(target), target)
                .into_iter()
                .all(|element| self.is_simple_binding_or_assignment_element(element));
        }
        ast::is_identifier(self.store_for(target), target)
    }

    fn is_simple_inlineable_expression(&self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        !ast::is_identifier(store, node)
            && matches!(
                store.kind(node),
                ast::Kind::StringLiteral
                    | ast::Kind::NumericLiteral
                    | ast::Kind::TrueKeyword
                    | ast::Kind::FalseKeyword
                    | ast::Kind::NullKeyword
            )
    }
}

/// Gets the elements of a BindingOrAssignmentPattern
fn get_elements_of_binding_or_assignment_pattern(
    source: &ast::AstStore,
    pattern: ast::Node,
) -> Vec<ast::Node> {
    match source.kind(pattern) {
        ast::Kind::ObjectBindingPattern
        | ast::Kind::ArrayBindingPattern
        | ast::Kind::ArrayLiteralExpression => source
            .elements(pattern)
            .map(|elements| elements.iter().collect())
            .unwrap_or_default(),
        ast::Kind::ObjectLiteralExpression => source
            .properties(pattern)
            .map(|properties| properties.iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn is_array_binding_elision(source: &ast::AstStore, element: ast::Node) -> bool {
    ast::is_omitted_expression(source, element)
        || (ast::is_binding_element(source, element) && source.name(element).is_none())
}

// BindingOrAssignmentElementAssignsToName checks if any target in a binding/assignment pattern assigns to the given name.
pub fn binding_or_assignment_element_assigns_to_name(
    source: &ast::AstStore,
    element: ast::Node,
    name: &str,
) -> bool {
    let Some(target) = get_target_of_binding_or_assignment_element(source, element) else {
        return false;
    };
    if is_binding_or_assignment_pattern(source, target) {
        binding_or_assignment_pattern_assigns_to_name(source, target, name)
    } else if ast::is_identifier(source, target) {
        source.text(target) == name
    } else {
        false
    }
}

fn binding_or_assignment_pattern_assigns_to_name(
    source: &ast::AstStore,
    pattern: ast::Node,
    name: &str,
) -> bool {
    get_elements_of_binding_or_assignment_pattern(source, pattern)
        .into_iter()
        .any(|element| binding_or_assignment_element_assigns_to_name(source, element, name))
}

// BindingOrAssignmentElementContainsNonLiteralComputedName checks if any element has a non-literal computed property name.
pub fn binding_or_assignment_element_contains_non_literal_computed_name(
    source: &ast::AstStore,
    element: ast::Node,
) -> bool {
    let property_name =
        ast::try_get_property_name_of_binding_or_assignment_element(source, element);
    if property_name.is_some_and(|property_name| {
        ast::is_computed_property_name(source, property_name)
            && source
                .expression(property_name)
                .is_some_and(|expression| !ast::is_literal_expression(source, expression))
    }) {
        return true;
    }
    let Some(target) = get_target_of_binding_or_assignment_element(source, element) else {
        return false;
    };
    is_binding_or_assignment_pattern(source, target)
        && binding_or_assignment_pattern_contains_non_literal_computed_name(source, target)
}

fn binding_or_assignment_pattern_contains_non_literal_computed_name(
    source: &ast::AstStore,
    pattern: ast::Node,
) -> bool {
    get_elements_of_binding_or_assignment_pattern(source, pattern)
        .into_iter()
        .any(|element| {
            binding_or_assignment_element_contains_non_literal_computed_name(source, element)
        })
}

// GetInitializerOfBindingOrAssignmentElement returns the initializer/default value of a binding or assignment element.
pub fn get_initializer_of_binding_or_assignment_element(
    source: &ast::AstStore,
    binding_element: Option<ast::Node>,
) -> Option<ast::Node> {
    let binding_element = binding_element?;
    match source.kind(binding_element) {
        ast::Kind::VariableDeclaration | ast::Kind::Parameter | ast::Kind::BindingElement => {
            source.initializer(binding_element)
        }
        ast::Kind::PropertyAssignment => {
            let initializer = source.initializer(binding_element)?;
            if ast::is_assignment_expression(source, initializer, true) {
                source.right(initializer)
            } else {
                None
            }
        }
        ast::Kind::ShorthandPropertyAssignment => {
            source.object_assignment_initializer(binding_element)
        }
        ast::Kind::BinaryExpression
            if ast::is_assignment_expression(source, binding_element, true) =>
        {
            source.right(binding_element)
        }
        ast::Kind::SpreadElement => source
            .expression(binding_element)
            .and_then(|expr| get_initializer_of_binding_or_assignment_element(source, Some(expr))),
        _ => None,
    }
}

fn get_target_of_binding_or_assignment_element(
    source: &ast::AstStore,
    element: ast::Node,
) -> Option<ast::Node> {
    match source.kind(element) {
        ast::Kind::VariableDeclaration | ast::Kind::Parameter | ast::Kind::BindingElement => {
            source.name(element)
        }
        ast::Kind::PropertyAssignment => source.initializer(element).and_then(|initializer| {
            get_target_of_binding_or_assignment_element(source, initializer)
        }),
        ast::Kind::ShorthandPropertyAssignment => source.name(element),
        ast::Kind::SpreadAssignment => source
            .expression(element)
            .and_then(|expression| get_target_of_binding_or_assignment_element(source, expression)),
        ast::Kind::BinaryExpression if ast::is_assignment_expression(source, element, true) => {
            source
                .left(element)
                .and_then(|left| get_target_of_binding_or_assignment_element(source, left))
        }
        ast::Kind::SpreadElement => source
            .expression(element)
            .and_then(|expression| get_target_of_binding_or_assignment_element(source, expression)),
        _ => Some(element),
    }
}

fn is_binding_or_assignment_pattern(source: &ast::AstStore, node: ast::Node) -> bool {
    matches!(
        source.kind(node),
        ast::Kind::ObjectBindingPattern
            | ast::Kind::ArrayBindingPattern
            | ast::Kind::ObjectLiteralExpression
            | ast::Kind::ArrayLiteralExpression
    )
}
