use ts_ast as ast;
use ts_core as core;
use ts_jsnum as jsnum;

#[derive(Clone, PartialEq)]
pub enum Value {
    None,
    String(String),
    Number(jsnum::Number),
    Bool(bool),
    PseudoBigInt(jsnum::PseudoBigInt),
}

impl Default for Value {
    fn default() -> Self {
        Self::None
    }
}

impl Value {
    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    pub fn as_number(&self) -> Option<jsnum::Number> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }
}

#[derive(Clone, Default)]
pub struct Result {
    pub value: Value,
    pub is_syntactically_string: bool,
    pub resolved_other_files: bool,
    pub has_external_references: bool,
}

impl Result {
    pub fn new(
        value: Value,
        is_syntactically_string: bool,
        resolved_other_files: bool,
        has_external_references: bool,
    ) -> Self {
        new_result(
            value,
            is_syntactically_string,
            resolved_other_files,
            has_external_references,
        )
    }
}

pub fn new_result(
    value: Value,
    is_syntactically_string: bool,
    resolved_other_files: bool,
    has_external_references: bool,
) -> Result {
    Result {
        value,
        is_syntactically_string,
        resolved_other_files,
        has_external_references,
    }
}

pub type Evaluator<'a> = Box<dyn Fn(&ast::AstStore, &ast::Node, &ast::Node) -> Result + 'a>;

pub fn new_evaluator<'a>(
    evaluate_entity: Evaluator<'a>,
    outer_expressions_to_skip: ast::OuterExpressionKinds,
) -> Evaluator<'a> {
    Box::new(
        move |store: &ast::AstStore, expr: &ast::Node, location: &ast::Node| {
            let mut evaluate_entity = |expr: &ast::Node, location: &ast::Node| {
                evaluate_entity.as_ref()(store, expr, location)
            };
            evaluate_expression(
                store,
                expr,
                location,
                &mut evaluate_entity,
                outer_expressions_to_skip,
            )
        },
    )
}

pub fn evaluate_expression(
    store: &ast::AstStore,
    expr: &ast::Node,
    location: &ast::Node,
    evaluate_entity: &mut dyn FnMut(&ast::Node, &ast::Node) -> Result,
    outer_expressions_to_skip: ast::OuterExpressionKinds,
) -> Result {
    let mut is_syntactically_string = false;
    let mut resolved_other_files = false;
    let mut has_external_references = false;
    // It's unclear when/whether we should consider skipping other kinds of outer expressions.
    // Type assertions intentionally break evaluation when evaluating literal types, such as:
    //     type T = `one ${"two" as any} three`; // string
    // But it's less clear whether such an assertion should break enum member evaluation:
    //     enum E {
    //       A = "one" as any
    //     }
    // SatisfiesExpressions and non-null assertions seem to have even less reason to break
    // emitting enum members as literals. However, these expressions also break Babel's
    // evaluation (but not esbuild's), and the isolatedModules errors we give depend on
    // our evaluation results, so we're currently being conservative so as to issue errors
    // on code that might break Babel.
    let expr = ast::skip_outer_expressions(
        store,
        *expr,
        outer_expressions_to_skip | ast::OEK_PARENTHESES,
    );
    match store.kind(expr) {
        ast::Kind::PrefixUnaryExpression => {
            let operand = store
                .operand(expr)
                .expect("prefix unary expression should have an operand");
            let result = evaluate_expression(
                store,
                &operand,
                location,
                evaluate_entity,
                outer_expressions_to_skip,
            );
            resolved_other_files = result.resolved_other_files;
            has_external_references = result.has_external_references;
            if let Value::Number(value) = result.value {
                match store
                    .operator(expr)
                    .expect("prefix unary expression should have an operator")
                {
                    ast::Kind::PlusToken => {
                        return new_result(
                            Value::Number(value),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::MinusToken => {
                        return new_result(
                            Value::Number(-value),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::TildeToken => {
                        return new_result(
                            Value::Number(value.bitwise_not()),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    _ => {}
                }
            }
        }
        ast::Kind::BinaryExpression => {
            let left_node = store
                .left(expr)
                .expect("binary expression should have a left operand");
            let right_node = store
                .right(expr)
                .expect("binary expression should have a right operand");
            let left = evaluate_expression(
                store,
                &left_node,
                location,
                evaluate_entity,
                outer_expressions_to_skip,
            );
            let right = evaluate_expression(
                store,
                &right_node,
                location,
                evaluate_entity,
                outer_expressions_to_skip,
            );
            let operator = store
                .operator_token(expr)
                .map(|operator| store.kind(operator))
                .expect("binary expression should have an operator token");
            is_syntactically_string = (left.is_syntactically_string
                || right.is_syntactically_string)
                && operator == ast::Kind::PlusToken;
            resolved_other_files = left.resolved_other_files || right.resolved_other_files;
            has_external_references = left.has_external_references || right.has_external_references;
            let left_num = match &left.value {
                Value::Number(value) => Some(*value),
                _ => None,
            };
            let right_num = match &right.value {
                Value::Number(value) => Some(*value),
                _ => None,
            };
            if let (Some(left_num), Some(right_num)) = (left_num, right_num) {
                match operator {
                    ast::Kind::BarToken => {
                        return new_result(
                            Value::Number(left_num.bitwise_or(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::AmpersandToken => {
                        return new_result(
                            Value::Number(left_num.bitwise_and(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::GreaterThanGreaterThanToken => {
                        return new_result(
                            Value::Number(left_num.signed_right_shift(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::GreaterThanGreaterThanGreaterThanToken => {
                        return new_result(
                            Value::Number(left_num.unsigned_right_shift(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::LessThanLessThanToken => {
                        return new_result(
                            Value::Number(left_num.left_shift(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::CaretToken => {
                        return new_result(
                            Value::Number(left_num.bitwise_xor(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::AsteriskToken => {
                        return new_result(
                            Value::Number(left_num * right_num),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::SlashToken => {
                        return new_result(
                            Value::Number(left_num / right_num),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::PlusToken => {
                        return new_result(
                            Value::Number(left_num + right_num),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::MinusToken => {
                        return new_result(
                            Value::Number(left_num - right_num),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::PercentToken => {
                        return new_result(
                            Value::Number(left_num.remainder(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    ast::Kind::AsteriskAsteriskToken => {
                        return new_result(
                            Value::Number(left_num.exponentiate(right_num)),
                            is_syntactically_string,
                            resolved_other_files,
                            has_external_references,
                        );
                    }
                    _ => {}
                }
            }
            let left_str = match &left.value {
                Value::String(value) => Some(value.clone()),
                _ => None,
            };
            let right_str = match &right.value {
                Value::String(value) => Some(value.clone()),
                _ => None,
            };
            if (left_str.is_some() || left_num.is_some())
                && (right_str.is_some() || right_num.is_some())
                && operator == ast::Kind::PlusToken
            {
                let left_str = left_str.unwrap_or_else(|| left_num.unwrap().to_string());
                let right_str = right_str.unwrap_or_else(|| right_num.unwrap().to_string());
                return new_result(
                    Value::String(left_str + &right_str),
                    is_syntactically_string,
                    resolved_other_files,
                    has_external_references,
                );
            }
        }
        ast::Kind::StringLiteral | ast::Kind::NoSubstitutionTemplateLiteral => {
            return new_result(
                Value::String(store.text(expr)),
                true, /*isSyntacticallyString*/
                false,
                false,
            );
        }
        ast::Kind::TemplateExpression => {
            return evaluate_template_expression(
                store,
                &expr,
                location,
                evaluate_entity,
                outer_expressions_to_skip,
            );
        }
        ast::Kind::NumericLiteral => {
            return new_result(
                Value::Number(jsnum::from_string(&store.text(expr))),
                false,
                false,
                false,
            );
        }
        ast::Kind::Identifier => {
            return evaluate_entity(&expr, location);
        }
        ast::Kind::ElementAccessExpression | ast::Kind::PropertyAccessExpression
            if store
                .expression(expr)
                .is_some_and(|expression| ast::is_entity_name_expression(store, expression)) =>
        {
            return evaluate_entity(&expr, location);
        }
        _ => {}
    }
    new_result(
        Value::None,
        is_syntactically_string,
        resolved_other_files,
        has_external_references,
    )
}

fn evaluate_template_expression(
    store: &ast::AstStore,
    expr: &ast::Node,
    location: &ast::Node,
    evaluate_entity: &mut dyn FnMut(&ast::Node, &ast::Node) -> Result,
    outer_expressions_to_skip: ast::OuterExpressionKinds,
) -> Result {
    let mut sb = String::new();
    sb.push_str(
        &store.text(
            store
                .head(*expr)
                .expect("template expression should have a head"),
        ),
    );
    let mut resolved_other_files = false;
    let mut has_external_references = false;
    let template_spans = store
        .template_spans(*expr)
        .expect("template expression should have template spans");
    for span_node in template_spans.iter() {
        let span_result = evaluate_expression(
            store,
            &store
                .expression(span_node)
                .expect("template span should have an expression"),
            location,
            evaluate_entity,
            outer_expressions_to_skip,
        );
        if matches!(span_result.value, Value::None) {
            return new_result(
                Value::None,
                true, /*isSyntacticallyString*/
                false,
                false,
            );
        }
        sb.push_str(&any_to_string(&span_result.value));
        sb.push_str(
            &store.text(
                store
                    .literal(span_node)
                    .expect("template span should have a literal"),
            ),
        );
        resolved_other_files = resolved_other_files || span_result.resolved_other_files;
        has_external_references = has_external_references || span_result.has_external_references;
    }
    new_result(
        Value::String(sb),
        true,
        resolved_other_files,
        has_external_references,
    )
}

pub fn any_to_string(v: &Value) -> String {
    match v {
        Value::String(v) => v.clone(),
        Value::Number(v) => v.to_string(),
        Value::Bool(v) => core::if_else(*v, "true".to_string(), "false".to_string()),
        Value::PseudoBigInt(v) => v.to_string(),
        Value::None => panic!("Unhandled case in AnyToString"),
    }
}

pub fn is_truthy(v: &Value) -> bool {
    match v {
        Value::String(v) => !v.is_empty(),
        Value::Number(v) => *v != jsnum::Number::from(0) && !v.is_nan(),
        Value::Bool(v) => *v,
        Value::PseudoBigInt(v) => *v != jsnum::PseudoBigInt::default(),
        Value::None => panic!("Unhandled case in IsTruthy"),
    }
}
