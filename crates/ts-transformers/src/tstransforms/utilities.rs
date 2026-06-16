use ts_ast as ast;

pub fn constant_expression_from_string(
    value: &str,
    factory: &mut ast::NodeFactory,
) -> ast::Expression {
    factory.new_string_literal(value.to_owned(), ast::TokenFlags::NONE)
}

pub fn constant_expression_from_number(
    value: f64,
    factory: &mut ast::NodeFactory,
) -> ast::Expression {
    if value.is_infinite() {
        return if value.is_sign_positive() {
            factory.new_identifier("Infinity".to_owned())
        } else {
            let infinity = factory.new_identifier("Infinity".to_owned());
            factory.new_prefix_unary_expression(ast::Kind::MinusToken, infinity)
        };
    }
    if value.is_nan() {
        return factory.new_identifier("NaN".to_owned());
    }
    if value < 0.0 {
        let positive = constant_expression_from_number(-value, factory);
        return factory.new_prefix_unary_expression(ast::Kind::MinusToken, positive);
    }
    factory.new_numeric_literal(value.to_string(), ast::TokenFlags::NONE)
}
