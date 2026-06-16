#![allow(dead_code)]

use std::ops::ControlFlow;

use ts_ast as ast;

use crate::{
    PseudoChecker, PseudoObjectElement, PseudoParameter, PseudoType, PseudoTypeKind,
    new_pseudo_get_accessor, new_pseudo_object_method, new_pseudo_parameter,
    new_pseudo_property_assignment, new_pseudo_set_accessor, new_pseudo_type_big_int_literal,
    new_pseudo_type_direct, new_pseudo_type_inferred, new_pseudo_type_inferred_with_errors,
    new_pseudo_type_maybe_const_location, new_pseudo_type_no_result,
    new_pseudo_type_numeric_literal, new_pseudo_type_object_literal,
    new_pseudo_type_single_call_signature, new_pseudo_type_string_literal, new_pseudo_type_tuple,
    new_pseudo_type_union, pseudo_type_big_int, pseudo_type_boolean, pseudo_type_false,
    pseudo_type_null, pseudo_type_number, pseudo_type_string, pseudo_type_true,
    pseudo_type_undefined,
};

fn kind(store: &ast::AstStore, node: &ast::Node) -> ast::Kind {
    store.kind(*node)
}

fn flags(store: &ast::AstStore, node: &ast::Node) -> ast::NodeFlags {
    store.flags(*node)
}

fn pos(store: &ast::AstStore, node: &ast::Node) -> i32 {
    store.loc(*node).pos()
}

fn end(store: &ast::AstStore, node: &ast::Node) -> i32 {
    store.loc(*node).end()
}

fn node_list_nodes(list: Option<ast::SourceNodeList<'_>>) -> Vec<ast::Node> {
    list.map(ast::SourceNodeList::nodes).unwrap_or_default()
}

fn member_or_property_nodes(store: &ast::AstStore, node: ast::Node) -> Vec<ast::Node> {
    node_list_nodes(store.members(node).or_else(|| store.properties(node)))
}

fn is_assertion_expression(store: &ast::AstStore, node: &ast::Node) -> bool {
    matches!(
        kind(store, node),
        ast::Kind::TypeAssertionExpression | ast::Kind::AsExpression
    )
}

fn is_variable_parameter_or_property(store: &ast::AstStore, node: &ast::Node) -> bool {
    matches!(
        kind(store, node),
        ast::Kind::VariableDeclaration
            | ast::Kind::Parameter
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
    )
}

fn has_modifier(store: &ast::AstStore, node: &ast::Node, modifier: ast::ModifierFlags) -> bool {
    ast::has_syntactic_modifier(store, *node, modifier)
}

fn is_const_type_reference(store: &ast::AstStore, node: &ast::Node) -> bool {
    kind(store, node) == ast::Kind::TypeReference
        && store
            .type_name(*node)
            .is_some_and(|name| ast::is_identifier(store, name) && store.text(name) == "const")
}

fn is_const_assertion(store: &ast::AstStore, node: &ast::Node) -> bool {
    is_assertion_expression(store, node)
        && store
            .r#type(*node)
            .is_some_and(|type_node| is_const_type_reference(store, &type_node))
}

fn is_primitive_literal_value(
    store: &ast::AstStore,
    node: &ast::Node,
    include_big_int: bool,
) -> bool {
    match kind(store, node) {
        ast::Kind::StringLiteral
        | ast::Kind::NoSubstitutionTemplateLiteral
        | ast::Kind::NumericLiteral
        | ast::Kind::TrueKeyword
        | ast::Kind::FalseKeyword
        | ast::Kind::NullKeyword => true,
        ast::Kind::BigIntLiteral => include_big_int,
        ast::Kind::PrefixUnaryExpression => store.operand(*node).is_some_and(|operand| {
            matches!(
                kind(store, &operand),
                ast::Kind::NumericLiteral | ast::Kind::BigIntLiteral
            )
        }),
        _ => false,
    }
}

impl PseudoChecker {
    pub fn get_return_type_of_signature(
        &self,
        store: &ast::AstStore,
        signature_node: &ast::Node,
    ) -> PseudoType {
        match kind(store, signature_node) {
            ast::Kind::GetAccessor => self.get_type_of_accessor(store, *signature_node),
            ast::Kind::MethodDeclaration
            | ast::Kind::FunctionDeclaration
            | ast::Kind::Constructor
            | ast::Kind::MethodSignature
            | ast::Kind::CallSignature
            | ast::Kind::ConstructSignature
            | ast::Kind::SetAccessor
            | ast::Kind::IndexSignature
            | ast::Kind::FunctionType
            | ast::Kind::ConstructorType
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction => self.create_return_from_signature(store, signature_node),
            _ => new_pseudo_type_no_result(signature_node.clone()),
        }
    }

    pub fn get_type_of_accessor(&self, store: &ast::AstStore, accessor: ast::Node) -> PseudoType {
        let annotated = self.type_from_accessor(store, &accessor);
        if annotated.kind == PseudoTypeKind::NoResult {
            self.infer_accessor_type(store, &accessor)
        } else {
            annotated
        }
    }

    pub fn get_type_of_expression(&self, store: &ast::AstStore, node: ast::Node) -> PseudoType {
        self.type_from_expression(store, &node)
    }

    pub fn get_type_of_declaration(&self, store: &ast::AstStore, node: ast::Node) -> PseudoType {
        self.get_type_of_declaration_with_single_variable_declaration(store, node, None)
    }

    pub fn get_type_of_declaration_with_single_variable_declaration(
        &self,
        store: &ast::AstStore,
        node: ast::Node,
        has_single_variable_declaration: Option<bool>,
    ) -> PseudoType {
        match kind(store, &node) {
            ast::Kind::Parameter => self.type_from_parameter(store, &node),
            ast::Kind::VariableDeclaration => {
                self.type_from_variable(store, &node, has_single_variable_declaration)
            }
            ast::Kind::PropertySignature | ast::Kind::PropertyDeclaration => {
                self.type_from_property(store, &node)
            }
            ast::Kind::BindingElement => new_pseudo_type_no_result(node),
            ast::Kind::ExportAssignment => store
                .expression(node)
                .map(|expression| self.type_from_expression(store, &expression))
                .unwrap_or_else(|| new_pseudo_type_no_result(node)),
            ast::Kind::PropertyAccessExpression
            | ast::Kind::ElementAccessExpression
            | ast::Kind::BinaryExpression => self.type_from_expando_property(store, &node),
            ast::Kind::PropertyAssignment | ast::Kind::ShorthandPropertyAssignment => {
                self.type_from_property_assignment(store, &node)
            }
            ast::Kind::CallExpression => new_pseudo_type_no_result(node),
            _ => new_pseudo_type_no_result(node),
        }
    }

    fn type_from_property_assignment(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        if let Some(annotation) = store.r#type(*node) {
            return new_pseudo_type_direct(annotation);
        }
        if kind(store, node) == ast::Kind::PropertyAssignment
            && let Some(init) = store.initializer(*node)
        {
            let expr = self.type_from_expression(store, &init);
            if expr.kind != PseudoTypeKind::Inferred
                || !expr.as_pseudo_type_inferred().error_nodes.is_empty()
            {
                return expr;
            }
        }
        new_pseudo_type_no_result(node.clone())
    }

    fn type_from_expando_property(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        if let Some(declared_type) = store.r#type(*node) {
            return new_pseudo_type_direct(declared_type);
        }
        new_pseudo_type_no_result(node.clone())
    }

    fn type_from_property(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        if let Some(type_node) = store.r#type(*node) {
            return new_pseudo_type_direct(type_node);
        }
        if ast::is_property_declaration(store, *node)
            && let Some(init) = store.initializer(*node)
            && !is_contextually_typed(store, *node)
        {
            if has_modifier(store, node, ast::ModifierFlags::READONLY)
                && kind(store, &init) == ast::Kind::TemplateExpression
            {
                return new_pseudo_type_no_result(node.clone());
            }
            let expr = self.type_from_expression(store, &init);
            if expr.kind != PseudoTypeKind::Inferred
                || !expr.as_pseudo_type_inferred().error_nodes.is_empty()
            {
                if expr.kind != PseudoTypeKind::Direct
                    && store
                        .postfix_token(*node)
                        .is_some_and(|postfix| kind(store, &postfix) == ast::Kind::QuestionToken)
                {
                    return add_undefined_if_definitely_required(store, expr);
                }
                return expr;
            }
        }
        new_pseudo_type_no_result(node.clone())
    }

    fn type_from_variable(
        &self,
        store: &ast::AstStore,
        node: &ast::Node,
        has_single_variable_declaration: Option<bool>,
    ) -> PseudoType {
        if let Some(type_node) = store.r#type(*node) {
            return new_pseudo_type_direct(type_node);
        }
        if let Some(init) = store.initializer(*node) {
            let has_single_variable_declaration =
                has_single_variable_declaration.unwrap_or_else(|| {
                    store
                        .parent(*node)
                        .and_then(|declaration_list| store.declarations(declaration_list))
                        .is_some_and(|declarations| declarations.len() == 1)
                });
            if has_single_variable_declaration && !is_contextually_typed(store, *node) {
                // TODO: Strada forces an inference fallback on `const` variables with template expression initializers, to leave space for template literal freshness in the future
                if ast::is_var_const(store, *node)
                    && kind(store, &init) == ast::Kind::TemplateExpression
                {
                    return new_pseudo_type_no_result(node.clone());
                }
                let expr = self.type_from_expression(store, &init);
                if expr.kind != PseudoTypeKind::Inferred
                    || !expr.as_pseudo_type_inferred().error_nodes.is_empty()
                {
                    return expr;
                }
                // fallback to NoResult if PseudoTypeKindInferred without error nodes
            }
        }
        new_pseudo_type_no_result(node.clone())
    }

    fn type_from_accessor(&self, store: &ast::AstStore, accessor: &ast::Node) -> PseudoType {
        let declarations = store
            .parent(*accessor)
            .map(|parent| member_or_property_nodes(store, parent))
            .unwrap_or_else(|| vec![*accessor]);
        let accessor_declarations =
            ast::get_all_accessor_declarations(store, &declarations, *accessor);
        if let Some(accessor_type) = self.get_type_annotation_from_all_accessor_declarations(
            store,
            accessor,
            &accessor_declarations,
        ) && !ast::is_type_predicate_node(store, accessor_type)
        {
            return new_pseudo_type_direct(accessor_type);
        }
        if let Some(get_accessor) = accessor_declarations.get_accessor {
            return self.create_return_from_signature(store, &get_accessor);
        }
        new_pseudo_type_no_result(accessor.clone())
    }

    fn infer_accessor_type(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        if kind(store, node) == ast::Kind::GetAccessor {
            self.create_return_from_signature(store, node)
        } else {
            new_pseudo_type_no_result(node.clone())
        }
    }

    fn get_type_annotation_from_all_accessor_declarations(
        &self,
        store: &ast::AstStore,
        node: &ast::Node,
        accessors: &ast::AllAccessorDeclarations,
    ) -> Option<ast::Node> {
        let mut accessor_type = self.get_type_annotation_from_accessor(store, Some(node));
        if accessor_type.is_none() && *node != accessors.first_accessor {
            accessor_type =
                self.get_type_annotation_from_accessor(store, Some(&accessors.first_accessor));
        }
        if accessor_type.is_none()
            && let Some(second_accessor) = accessors.second_accessor
            && *node != second_accessor
        {
            accessor_type = self.get_type_annotation_from_accessor(store, Some(&second_accessor));
        }
        accessor_type
    }

    fn get_type_annotation_from_accessor(
        &self,
        store: &ast::AstStore,
        node: Option<&ast::Node>,
    ) -> Option<ast::Node> {
        let node = node?;
        if kind(store, node) == ast::Kind::GetAccessor {
            return store.r#type(*node);
        }
        if kind(store, node) != ast::Kind::SetAccessor {
            return None;
        }
        store
            .parameters(*node)
            .and_then(|parameters| parameters.first())
            .and_then(|parameter| store.r#type(parameter))
    }

    fn create_return_from_signature(
        &self,
        store: &ast::AstStore,
        fn_node: &ast::Node,
    ) -> PseudoType {
        if let Some(return_type) = store
            .r#type(*fn_node)
            .filter(|return_type| !ast::node_is_missing(store, Some(*return_type)))
        {
            return new_pseudo_type_direct(return_type);
        }
        if is_value_signature_declaration(store, *fn_node) {
            return self.type_from_single_return_expression(store, fn_node);
        }
        new_pseudo_type_no_result(fn_node.clone())
    }

    fn type_from_single_return_expression(
        &self,
        store: &ast::AstStore,
        fn_node: &ast::Node,
    ) -> PseudoType {
        let Some(body) = store.body(*fn_node) else {
            return new_pseudo_type_no_result(fn_node.clone());
        };
        let flags = ast::get_function_flags(store, Some(*fn_node));
        if flags & ast::FUNCTION_FLAGS_ASYNC_GENERATOR != 0 {
            return new_pseudo_type_no_result(fn_node.clone());
        }

        let candidate_expr = if kind(store, &body) == ast::Kind::Block {
            single_return_expression_from_block(store, &body)
        } else {
            Some(body)
        };

        if let Some(candidate_expr) = candidate_expr {
            if is_contextually_typed(store, candidate_expr) {
                if matches!(
                    kind(store, &candidate_expr),
                    ast::Kind::TypeAssertionExpression | ast::Kind::AsExpression
                ) && let Some(type_node) = store.r#type(candidate_expr)
                    && !is_const_type_reference(store, &type_node)
                {
                    return new_pseudo_type_direct(type_node);
                }
            } else {
                return self.type_from_expression(store, &candidate_expr);
            }
        }
        new_pseudo_type_no_result(fn_node.clone())
    }

    fn type_from_expression(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        match kind(store, node) {
            ast::Kind::OmittedExpression => pseudo_type_undefined(),
            ast::Kind::ParenthesizedExpression => store
                .expression(*node)
                .map(|expression| self.type_from_expression(store, &expression))
                .unwrap_or_else(|| new_pseudo_type_inferred(node.clone())),
            ast::Kind::Identifier if store.text(*node) == "undefined" => pseudo_type_undefined(),
            ast::Kind::NullKeyword => pseudo_type_null(),
            ast::Kind::TypeAssertionExpression | ast::Kind::AsExpression => {
                match (store.expression(*node), store.r#type(*node)) {
                    (Some(expression), Some(type_node)) => {
                        self.type_from_type_assertion(store, &expression, &type_node)
                    }
                    _ => new_pseudo_type_inferred(node.clone()),
                }
            }
            ast::Kind::PrefixUnaryExpression if is_primitive_literal_value(store, node, true) => {
                self.type_from_primitive_literal_prefix(store, node)
            }
            ast::Kind::ClassExpression => new_pseudo_type_inferred(node.clone()),
            ast::Kind::TemplateExpression => {
                if is_in_const_context(store, *node) {
                    new_pseudo_type_inferred(node.clone())
                } else {
                    new_pseudo_type_maybe_const_location(
                        node.clone(),
                        new_pseudo_type_inferred(node.clone()),
                        pseudo_type_string(),
                    )
                }
            }
            ast::Kind::NumericLiteral => new_pseudo_type_maybe_const_location(
                node.clone(),
                new_pseudo_type_numeric_literal(node.clone()),
                pseudo_type_number(),
            ),
            ast::Kind::NoSubstitutionTemplateLiteral | ast::Kind::StringLiteral => {
                new_pseudo_type_maybe_const_location(
                    node.clone(),
                    new_pseudo_type_string_literal(node.clone()),
                    pseudo_type_string(),
                )
            }
            ast::Kind::BigIntLiteral => new_pseudo_type_maybe_const_location(
                node.clone(),
                new_pseudo_type_big_int_literal(node.clone()),
                pseudo_type_big_int(),
            ),
            ast::Kind::TrueKeyword => new_pseudo_type_maybe_const_location(
                node.clone(),
                pseudo_type_true(),
                pseudo_type_boolean(),
            ),
            ast::Kind::FalseKeyword => new_pseudo_type_maybe_const_location(
                node.clone(),
                pseudo_type_false(),
                pseudo_type_boolean(),
            ),
            ast::Kind::ArrayLiteralExpression => self.type_from_array_literal(store, node),
            ast::Kind::ObjectLiteralExpression => self.type_from_object_literal(store, node),
            ast::Kind::ArrowFunction | ast::Kind::FunctionExpression => {
                self.type_from_function_like_expression(store, node)
            }
            _ => new_pseudo_type_inferred(node.clone()),
        }
    }

    fn type_from_object_literal(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        if let Some(error_nodes) = self.can_get_type_from_object_literal(store, node) {
            return new_pseudo_type_inferred_with_errors(node.clone(), error_nodes);
        }
        let properties = node_list_nodes(store.properties(*node));
        if properties.is_empty() {
            return new_pseudo_type_object_literal(node.clone(), Vec::new());
        }

        let mut results = Vec::with_capacity(properties.len());
        for property in properties {
            match kind(store, &property) {
                ast::Kind::PropertyAssignment => {
                    let Some(name) = store.name(property) else {
                        return new_pseudo_type_inferred_with_errors(node.clone(), vec![property]);
                    };
                    let optional = store
                        .postfix_token(property)
                        .is_some_and(|postfix| kind(store, &postfix) == ast::Kind::QuestionToken);
                    let type_ = store
                        .initializer(property)
                        .map(|initializer| self.type_from_expression(store, &initializer))
                        .unwrap_or_else(|| new_pseudo_type_inferred(property.clone()));
                    results.push(new_pseudo_property_assignment(false, name, optional, type_));
                }
                ast::Kind::MethodDeclaration => {
                    let Some(name) = store.name(property) else {
                        return new_pseudo_type_inferred_with_errors(node.clone(), vec![property]);
                    };
                    let optional = store
                        .postfix_token(property)
                        .is_some_and(|postfix| kind(store, &postfix) == ast::Kind::QuestionToken);
                    if let Some(full_signature) = store.full_signature(property) {
                        results.push(new_pseudo_property_assignment(
                            false,
                            name,
                            optional,
                            new_pseudo_type_direct(full_signature),
                        ));
                    } else {
                        results.push(new_pseudo_object_method(
                            property.clone(),
                            name,
                            optional,
                            self.clone_type_parameters(store, &property),
                            self.clone_parameters(
                                store,
                                &node_list_nodes(store.parameters(property)),
                            ),
                            self.create_return_from_signature(store, &property),
                        ));
                    }
                }
                ast::Kind::GetAccessor | ast::Kind::SetAccessor => {
                    if let Some(name) = store.name(property)
                        && let Some(member) = self.get_accessor_member(store, &property, name)
                    {
                        results.push(member);
                    }
                }
                _ => {}
            }
        }
        new_pseudo_type_object_literal(node.clone(), results)
    }

    fn get_accessor_member(
        &self,
        store: &ast::AstStore,
        accessor: &ast::Node,
        name: ast::Node,
    ) -> Option<PseudoObjectElement> {
        let declarations = store
            .parent(*accessor)
            .map(|parent| member_or_property_nodes(store, parent))
            .unwrap_or_else(|| vec![*accessor]);
        let accessors = ast::get_all_accessor_declarations(store, &declarations, *accessor);

        let get_type = accessors
            .get_accessor
            .as_ref()
            .and_then(|get| store.r#type(*get));
        let set_type = accessors.set_accessor.as_ref().and_then(|set| {
            store
                .parameters(*set)
                .and_then(|parameters| parameters.first())
                .and_then(|param| store.r#type(param))
        });
        if get_type.is_some() && set_type.is_some() {
            if kind(store, accessor) == ast::Kind::GetAccessor {
                return Some(new_pseudo_get_accessor(
                    accessor.clone(),
                    name,
                    false,
                    self.type_from_accessor(store, accessor),
                ));
            }
            let parameter = self
                .clone_parameters(store, &node_list_nodes(store.parameters(*accessor)))
                .into_iter()
                .next()?;
            return Some(new_pseudo_set_accessor(
                accessor.clone(),
                name,
                false,
                parameter,
            ));
        }

        if pos(store, accessor) == pos(store, &accessors.first_accessor)
            && kind(store, accessor) == kind(store, &accessors.first_accessor)
        {
            let readonly = kind(store, accessor) == ast::Kind::GetAccessor
                && accessors.second_accessor.is_none();
            return Some(new_pseudo_property_assignment(
                readonly,
                name,
                false,
                self.type_from_accessor(store, accessor),
            ));
        }
        None
    }

    fn can_get_type_from_object_literal(
        &self,
        store: &ast::AstStore,
        node: &ast::Node,
    ) -> Option<Vec<ast::Node>> {
        let properties = node_list_nodes(store.properties(*node));
        if properties.is_empty() {
            return None;
        }
        let mut error_nodes = Vec::new();
        for property in properties {
            if flags(store, &property).intersects(ast::NodeFlags::THIS_NODE_HAS_ERROR) {
                error_nodes.push(property);
                continue;
            }
            if matches!(
                kind(store, &property),
                ast::Kind::ShorthandPropertyAssignment | ast::Kind::SpreadAssignment
            ) {
                error_nodes.push(property);
                continue;
            }
            let Some(name) = store.name(property) else {
                error_nodes.push(property);
                continue;
            };
            if flags(store, &name).intersects(ast::NodeFlags::THIS_NODE_HAS_ERROR) {
                error_nodes.push(name);
                continue;
            }
            if kind(store, &name) == ast::Kind::PrivateIdentifier {
                error_nodes.push(property);
                continue;
            }
            if kind(store, &name) == ast::Kind::ComputedPropertyName {
                let Some(expression) = store.expression(name) else {
                    error_nodes.push(name);
                    continue;
                };
                if !is_primitive_literal_value(store, &expression, false) {
                    error_nodes.push(name);
                }
            }
        }
        if error_nodes.is_empty() {
            None
        } else {
            Some(error_nodes)
        }
    }

    fn type_from_array_literal(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        if let Some(error_nodes) = self.can_get_type_from_array_literal(store, node) {
            return new_pseudo_type_inferred_with_errors(node.clone(), error_nodes);
        }
        if !is_in_const_context(store, *node) {
            return new_pseudo_type_inferred(node.clone());
        }
        if is_in_const_context(store, *node) && is_contextually_typed(store, *node) {
            return new_pseudo_type_inferred(node.clone());
        }
        let elements = node_list_nodes(store.elements(*node))
            .into_iter()
            .map(|element| self.type_from_expression(store, &element))
            .collect();
        new_pseudo_type_tuple(elements)
    }

    fn can_get_type_from_array_literal(
        &self,
        store: &ast::AstStore,
        node: &ast::Node,
    ) -> Option<Vec<ast::Node>> {
        if !is_in_const_context(store, *node) {
            return Some(vec![node.clone()]);
        }
        node_list_nodes(store.elements(*node))
            .into_iter()
            .find(|element| kind(store, element) == ast::Kind::SpreadElement)
            .map(|element| vec![element])
    }

    fn type_from_primitive_literal_prefix(
        &self,
        store: &ast::AstStore,
        node: &ast::Node,
    ) -> PseudoType {
        let inner = store
            .operand(*node)
            .expect("PrefixUnaryExpression should have an operand");
        let expr = if store
            .operator(*node)
            .expect("PrefixUnaryExpression should have an operator")
            == ast::Kind::PlusToken
        {
            inner
        } else {
            node.clone()
        };
        match kind(store, &inner) {
            ast::Kind::BigIntLiteral => new_pseudo_type_maybe_const_location(
                node.clone(),
                new_pseudo_type_big_int_literal(expr),
                pseudo_type_big_int(),
            ),
            ast::Kind::NumericLiteral => new_pseudo_type_maybe_const_location(
                node.clone(),
                new_pseudo_type_numeric_literal(expr),
                pseudo_type_number(),
            ),
            _ => new_pseudo_type_inferred(node.clone()),
        }
    }

    fn type_from_type_assertion(
        &self,
        store: &ast::AstStore,
        expression: &ast::Node,
        type_node: &ast::Node,
    ) -> PseudoType {
        if is_const_type_reference(store, type_node) {
            self.type_from_expression(store, expression)
        } else {
            new_pseudo_type_direct(type_node.clone())
        }
    }

    fn type_from_function_like_expression(
        &self,
        store: &ast::AstStore,
        node: &ast::Node,
    ) -> PseudoType {
        if let Some(full_signature) = store.full_signature(*node) {
            return new_pseudo_type_direct(full_signature);
        }
        let return_type = self.create_return_from_signature(store, node);
        if return_type.kind == PseudoTypeKind::NoResult {
            return new_pseudo_type_inferred(node.clone());
        }
        new_pseudo_type_single_call_signature(
            node.clone(),
            self.clone_parameters(store, &node_list_nodes(store.parameters(*node))),
            self.clone_type_parameters(store, node),
            return_type,
        )
    }

    fn clone_type_parameters(&self, store: &ast::AstStore, node: &ast::Node) -> Vec<ast::Node> {
        store
            .type_parameters(*node)
            .map(|type_parameters| type_parameters.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|node| kind(store, node) == ast::Kind::TypeParameter)
            .collect()
    }

    fn type_from_parameter(&self, store: &ast::AstStore, node: &ast::Node) -> PseudoType {
        if let Some(parent) = store.parent(*node)
            && kind(store, &parent) == ast::Kind::SetAccessor
        {
            return self.get_type_of_accessor(store, parent);
        }
        if store.initializer(*node).is_none() {
            if let Some(type_node) = store.r#type(*node) {
                return new_pseudo_type_direct(type_node);
            }
            return new_pseudo_type_no_result(node.clone());
        }
        let params = store
            .parent(*node)
            .map(|parent| node_list_nodes(store.parameters(parent)))
            .unwrap_or_default();
        let self_idx = params
            .iter()
            .position(|param| same_syntax_node(store, param, node))
            .unwrap_or(0);
        let last_required = last_required_param_index(store, &params);
        self.type_from_parameter_worker(store, node, self_idx, last_required)
    }

    fn type_from_parameter_worker(
        &self,
        store: &ast::AstStore,
        node: &ast::Node,
        self_idx: usize,
        last_required: usize,
    ) -> PseudoType {
        if let Some(parent) = store.parent(*node)
            && kind(store, &parent) == ast::Kind::SetAccessor
        {
            return self.get_type_of_accessor(store, parent);
        }
        let has_required_after = self_idx < last_required.saturating_sub(1);
        if let Some(declared_type) = store.r#type(*node) {
            let result = new_pseudo_type_direct(declared_type);
            if self.strict_null_checks() && store.initializer(*node).is_some() && has_required_after
            {
                return add_undefined_if_definitely_required(store, result);
            }
            return result;
        }
        if let Some(initializer) = store.initializer(*node)
            && store
                .name(*node)
                .is_some_and(|name| ast::is_identifier(store, name))
            && !is_contextually_typed(store, *node)
        {
            let expr = self.type_from_expression(store, &initializer);
            if !self.strict_null_checks() || !has_required_after {
                return expr;
            }
            return add_undefined_if_definitely_required(store, expr);
        }
        new_pseudo_type_no_result(node.clone())
    }

    fn clone_parameters(&self, store: &ast::AstStore, nodes: &[ast::Node]) -> Vec<PseudoParameter> {
        let last_required = last_required_param_index(store, nodes);
        nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let name = store.name(*node)?;
                let mut optional = store.question_token(*node).is_some();
                if !optional && store.initializer(*node).is_some() {
                    optional = index >= last_required.saturating_sub(1);
                }
                Some(new_pseudo_parameter(
                    store.dot_dot_dot_token(*node).is_some(),
                    name,
                    optional,
                    self.type_from_parameter_worker(store, node, index, last_required),
                ))
            })
            .collect()
    }
}

pub fn is_value_signature_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    matches!(
        kind(store, &node),
        ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::FunctionDeclaration
            | ast::Kind::Constructor
    )
}

pub fn is_const_context_propagating_kind(_kind: ast::Kind) -> bool {
    matches!(
        _kind,
        ast::Kind::ArrayLiteralExpression
            | ast::Kind::ObjectLiteralExpression
            | ast::Kind::ParenthesizedExpression
            | ast::Kind::SpreadElement
            | ast::Kind::PropertyAssignment
            | ast::Kind::ShorthandPropertyAssignment
            | ast::Kind::TemplateSpan
            | ast::Kind::PrefixUnaryExpression
    )
}

pub fn is_in_const_context(store: &ast::AstStore, node: ast::Node) -> bool {
    let mut current = store.parent(node);
    while let Some(node) = current {
        if is_assertion_expression(store, &node)
            || !is_const_context_propagating_kind(kind(store, &node))
        {
            return is_const_assertion(store, &node);
        }
        current = store.parent(node);
    }
    false
}

pub fn is_undefined_pseudo_type(t: &PseudoType) -> bool {
    t.kind == PseudoTypeKind::Undefined
        || matches!(
            t.kind,
            PseudoTypeKind::MaybeConstLocation
                if is_undefined_pseudo_type(&t.as_pseudo_type_maybe_const_location().const_type)
        )
}

pub fn type_node_could_refer_to_undefined(store: &ast::AstStore, node: ast::Node) -> bool {
    let mut node = node;
    while kind(store, &node) == ast::Kind::ParenthesizedType {
        let Some(inner) = store.r#type(node) else {
            return true;
        };
        node = inner;
    }

    match kind(store, &node) {
        ast::Kind::TypeReference
        | ast::Kind::IndexedAccessType
        | ast::Kind::TypeQuery
        | ast::Kind::OptionalType
        | ast::Kind::RestType
        | ast::Kind::ImportType
        | ast::Kind::ConditionalType
        | ast::Kind::TypeOperator
        | ast::Kind::TypePredicate
        | ast::Kind::UndefinedKeyword => true,
        ast::Kind::IntersectionType | ast::Kind::UnionType => {
            store.types(node).is_some_and(|types| {
                types
                    .iter()
                    .any(|node| type_node_could_refer_to_undefined(store, node))
            })
        }
        _ => false,
    }
}

pub fn could_already_refer_to_undefined_type(store: &ast::AstStore, t: &PseudoType) -> bool {
    match t.kind {
        PseudoTypeKind::NoResult | PseudoTypeKind::Inferred => true,
        PseudoTypeKind::Undefined => true,
        PseudoTypeKind::MaybeConstLocation => {
            let mc = t.as_pseudo_type_maybe_const_location();
            could_already_refer_to_undefined_type(store, &mc.regular_type)
        }
        PseudoTypeKind::Direct => {
            type_node_could_refer_to_undefined(store, t.as_pseudo_type_direct().type_node)
        }
        PseudoTypeKind::Union => t
            .as_pseudo_type_union()
            .types
            .iter()
            .any(|t| could_already_refer_to_undefined_type(store, t)),
        _ => false,
    }
}

pub fn is_optional_initialized_or_rest_parameter(store: &ast::AstStore, node: ast::Node) -> bool {
    if kind(store, &node) != ast::Kind::Parameter {
        return false;
    }
    store.dot_dot_dot_token(node).is_some()
        || store.initializer(node).is_some()
        || store.question_token(node).is_some()
}

pub fn last_required_param_index(store: &ast::AstStore, _params: &[ast::Node]) -> usize {
    _params
        .iter()
        .rposition(|param| !is_optional_initialized_or_rest_parameter(store, *param))
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn same_syntax_node(store: &ast::AstStore, left: &ast::Node, right: &ast::Node) -> bool {
    kind(store, left) == kind(store, right)
        && pos(store, left) == pos(store, right)
        && end(store, left) == end(store, right)
}

fn single_return_expression_from_block(
    store: &ast::AstStore,
    body: &ast::Node,
) -> Option<ast::Node> {
    fn traverse(
        store: &ast::AstStore,
        node: &ast::Node,
        body: &ast::Node,
        candidate: &mut Option<ast::Node>,
        invalid: &mut bool,
    ) -> bool {
        if *invalid {
            return true;
        }
        match kind(store, node) {
            ast::Kind::ReturnStatement => {
                if store
                    .parent(*node)
                    .is_none_or(|parent| !same_syntax_node(store, &parent, body))
                    || candidate.is_some()
                {
                    *candidate = None;
                    *invalid = true;
                    return true;
                }
                *candidate = store.expression(*node);
                false
            }
            ast::Kind::CaseBlock
            | ast::Kind::Block
            | ast::Kind::IfStatement
            | ast::Kind::DoStatement
            | ast::Kind::WhileStatement
            | ast::Kind::ForStatement
            | ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement
            | ast::Kind::WithStatement
            | ast::Kind::SwitchStatement
            | ast::Kind::CaseClause
            | ast::Kind::DefaultClause
            | ast::Kind::LabeledStatement
            | ast::Kind::TryStatement
            | ast::Kind::CatchClause => {
                let result = store.for_each_present_child(*node, |child| {
                    if traverse(store, &child, body, candidate, invalid) {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                });
                matches!(result, ControlFlow::Break(()))
            }
            _ => false,
        }
    }

    let mut candidate = None;
    let mut invalid = false;
    traverse(store, body, body, &mut candidate, &mut invalid);
    if invalid { None } else { candidate }
}

pub fn add_undefined_if_definitely_required(store: &ast::AstStore, expr: PseudoType) -> PseudoType {
    if could_already_refer_to_undefined_type(store, &expr) {
        expr
    } else {
        new_pseudo_type_union(vec![expr, pseudo_type_undefined()])
    }
}

pub fn is_contextually_typed(store: &ast::AstStore, node: ast::Node) -> bool {
    let mut current = store.parent(node);
    while let Some(node) = current {
        if ast::is_call_expression(store, node)
            || kind(store, &node) == ast::Kind::SatisfiesExpression
        {
            return true;
        }
        if (is_variable_parameter_or_property(store, &node)
            || is_assertion_expression(store, &node))
            && store.r#type(node).is_some()
            && !is_const_assertion(store, &node)
        {
            return true;
        }
        if matches!(
            kind(store, &node),
            ast::Kind::JsxElement | ast::Kind::JsxSelfClosingElement | ast::Kind::JsxExpression
        ) {
            return true;
        }
        current = store.parent(node);
    }
    false
}
