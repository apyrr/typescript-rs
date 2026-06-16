pub fn is_class_this_assignment_block_shape(
    is_class_static_block: bool,
    statement_count: usize,
    only_statement_is_expression_statement: bool,
    expression_is_simple_assignment: bool,
    left_is_identifier: bool,
    left_matches_emit_context_class_this: bool,
    right_is_this_keyword: bool,
) -> bool {
    is_class_static_block
        && statement_count == 1
        && only_statement_is_expression_statement
        && expression_is_simple_assignment
        && left_is_identifier
        && left_matches_emit_context_class_this
        && right_is_this_keyword
}
