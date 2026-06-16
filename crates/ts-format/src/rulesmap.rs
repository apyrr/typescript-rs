use std::sync::OnceLock;

use ts_ast as ast;

use crate::{FormattingContext, RuleAction, RuleImpl, get_all_rules, lsutil, rulecontext::*};

pub fn get_rules(context: &mut FormattingContext, mut rules: Vec<RuleImpl>) -> Vec<RuleImpl> {
    let bucket = &get_rules_map()[get_rule_bucket_index(
        context.current_token_span.kind,
        context.next_token_span.kind,
    )];
    if !bucket.is_empty() {
        let mut rule_action_mask = RuleAction::NONE;
        'outer: for rule in bucket {
            let accept_rule_actions = !get_rule_action_exclusion(rule_action_mask).0;
            if rule.action().0 & accept_rule_actions != 0 {
                let preds = rule.context();
                for p in preds {
                    if !p(context) {
                        continue 'outer;
                    }
                }
                for name in rule.context_names() {
                    if !rule_context_name_matches(name, context) {
                        continue 'outer;
                    }
                }
                rules.push(rule.clone());
                rule_action_mask = RuleAction(rule_action_mask.0 | rule.action().0);
            }
        }
        return rules;
    }
    rules
}

fn rule_context_name_matches(name: &str, context: &mut FormattingContext) -> bool {
    match name {
        "isAfterCodeBlockContext" => is_after_code_block_context(context),
        "isArrowFunctionContext" => is_arrow_function_context(context),
        "isBeforeBlockContext" => is_before_block_context(context),
        "isBeforeMultilineBlockContext" => is_before_multiline_block_context(context),
        "isBinaryOpContext" => is_binary_op_context(context),
        "isBraceWrappedContext" => is_brace_wrapped_context(context),
        "isConditionalOperatorContext" => is_conditional_operator_context(context),
        "isConstructorSignatureContext" => is_constructor_signature_context(context),
        "isControlDeclContext" => is_control_decl_context(context),
        "isEndOfDecoratorContextOnSameLine" => is_end_of_decorator_context_on_same_line(context),
        "isForContext" => is_for_context(context),
        "isFunctionCallOrNewContext" => is_function_call_or_new_context(context),
        "isFunctionDeclarationOrFunctionExpressionContext" => {
            is_function_declaration_or_function_expression_context(context)
        }
        "isFunctionDeclContext" => is_function_decl_context(context),
        "isImportTypeContext" => is_import_type_context(context),
        "isJsxAttributeContext" => is_jsx_attribute_context(context),
        "isJsxExpressionContext" => is_jsx_expression_context(context),
        "isJsxSelfClosingElementContext" => is_jsx_self_closing_element_context(context),
        "isModuleDeclContext" => is_module_decl_context(context),
        "isMultilineBlockContext" => is_multiline_block_context(context),
        "isNextTokenNotCloseBracket" => is_next_token_not_close_bracket(context),
        "isNextTokenNotCloseParen" => is_next_token_not_close_paren(context),
        "isNextTokenParentJsxAttribute" => is_next_token_parent_jsx_attribute(context),
        "isNextTokenParentJsxNamespacedName" => is_next_token_parent_jsx_namespaced_name(context),
        "isNextTokenParentNotJsxNamespacedName" => {
            is_next_token_parent_not_jsx_namespaced_name(context)
        }
        "isNonJsxElementOrFragmentContext" => is_non_jsx_element_or_fragment_context(context),
        "isNonJsxSameLineTokenContext" => is_non_jsx_same_line_token_context(context),
        "isNonJsxTextContext" => is_non_jsx_text_context(context),
        "isNonNullAssertionContext" => is_non_null_assertion_context(context),
        "isNonOptionalPropertyContext" => is_non_optional_property_context(context),
        "isNonTypeAssertionContext" => is_non_type_assertion_context(context),
        "isNotBeforeBlockInFunctionDeclarationContext" => {
            is_not_before_block_in_function_declaration_context(context)
        }
        "isNotBinaryOpContext" => is_not_binary_op_context(context),
        "isNotForContext" => is_not_for_context(context),
        "isNotFormatOnEnter" => is_not_format_on_enter(context),
        "isNotFunctionDeclContext" => is_not_function_decl_context(context),
        "isNotPropertyAccessOnIntegerLiteral" => is_not_property_access_on_integer_literal(context),
        "isNotStatementConditionContext" => is_not_statement_condition_context(context),
        "isNotTypeAnnotationContext" => is_not_type_annotation_context(context),
        "isObjectContext" => is_object_context(context),
        "isObjectTypeContext" => is_object_type_context(context),
        "isPreviousTokenNotComma" => is_previous_token_not_comma(context),
        "isSameLineTokenOrBeforeBlockContext" => {
            is_same_line_token_or_before_block_context(context)
        }
        "isSemicolonDeletionContext" => is_semicolon_deletion_context(context),
        "isSemicolonInsertionContext" => is_semicolon_insertion_context(context),
        "isStartOfVariableDeclarationList" => is_start_of_variable_declaration_list(context),
        "isTypeAnnotationContext" => is_type_annotation_context(context),
        "isTypeArgumentOrParameterOrAssertionContext" => {
            is_type_argument_or_parameter_or_assertion_context(context)
        }
        "isTypeAssertionContext" => is_type_assertion_context(context),
        "isTypeScriptDeclWithBlockContext" => is_type_script_decl_with_block_context(context),
        "isVoidOpContext" => is_void_op_context(context),
        "isYieldOrYieldStarWithOperand" => is_yield_or_yield_star_with_operand(context),
        "isOptionDisabled(insertSpaceAfterOpeningAndBeforeClosingEmptyBracesOption)" => {
            insert_space_after_opening_and_before_closing_empty_braces_option(
                context.options.clone(),
            )
            .is_false()
        }
        "isOptionDisabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracesOption)" => {
            insert_space_after_opening_and_before_closing_nonempty_braces_option(
                context.options.clone(),
            )
            .is_false()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterCommaDelimiterOption)" => {
            insert_space_after_comma_delimiter_option(context.options.clone()).is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterConstructorOption)" => {
            insert_space_after_constructor_option(context.options.clone()).is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterFunctionKeywordForAnonymousFunctionsOption)" => {
            insert_space_after_function_keyword_for_anonymous_functions_option(
                context.options.clone(),
            )
            .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterKeywordsInControlFlowStatementsOption)" => {
            insert_space_after_keywords_in_control_flow_statements_option(context.options.clone())
                .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBracesOption)" => {
            insert_space_after_opening_and_before_closing_jsx_expression_braces_option(
                context.options.clone(),
            )
            .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracketsOption)" => {
            insert_space_after_opening_and_before_closing_nonempty_brackets_option(
                context.options.clone(),
            )
            .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesisOption)" => {
            insert_space_after_opening_and_before_closing_nonempty_parenthesis_option(
                context.options.clone(),
            )
            .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingTemplateStringBracesOption)" => {
            insert_space_after_opening_and_before_closing_template_string_braces_option(
                context.options.clone(),
            )
            .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterSemicolonInForStatementsOption)" => {
            insert_space_after_semicolon_in_for_statements_option(context.options.clone())
                .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceAfterTypeAssertionOption)" => {
            insert_space_after_type_assertion_option(context.options.clone()).is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceBeforeAndAfterBinaryOperatorsOption)" => {
            insert_space_before_and_after_binary_operators_option(context.options.clone())
                .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceBeforeFunctionParenthesisOption)" => {
            insert_space_before_function_parenthesis_option(context.options.clone())
                .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefined(insertSpaceBeforeTypeAnnotationOption)" => {
            insert_space_before_type_annotation_option(context.options.clone())
                .is_false_or_unknown()
        }
        "isOptionDisabledOrUndefinedOrTokensOnSameLine(placeOpenBraceOnNewLineForControlBlocksOption)" => {
            place_open_brace_on_new_line_for_control_blocks_option(context.options.clone())
                .is_false_or_unknown()
                || context.tokens_are_on_same_line()
        }
        "isOptionDisabledOrUndefinedOrTokensOnSameLine(placeOpenBraceOnNewLineForFunctionsOption)" => {
            place_open_brace_on_new_line_for_functions_option(context.options.clone())
                .is_false_or_unknown()
                || context.tokens_are_on_same_line()
        }
        "isOptionEnabled(insertSpaceAfterCommaDelimiterOption)" => {
            insert_space_after_comma_delimiter_option(context.options.clone()).is_true()
        }
        "isOptionEnabled(insertSpaceAfterConstructorOption)" => {
            insert_space_after_constructor_option(context.options.clone()).is_true()
        }
        "isOptionEnabled(insertSpaceAfterFunctionKeywordForAnonymousFunctionsOption)" => {
            insert_space_after_function_keyword_for_anonymous_functions_option(
                context.options.clone(),
            )
            .is_true()
        }
        "isOptionEnabled(insertSpaceAfterKeywordsInControlFlowStatementsOption)" => {
            insert_space_after_keywords_in_control_flow_statements_option(context.options.clone())
                .is_true()
        }
        "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingEmptyBracesOption)" => {
            insert_space_after_opening_and_before_closing_empty_braces_option(
                context.options.clone(),
            )
            .is_true()
        }
        "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingJsxExpressionBracesOption)" => {
            insert_space_after_opening_and_before_closing_jsx_expression_braces_option(
                context.options.clone(),
            )
            .is_true()
        }
        "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracketsOption)" => {
            insert_space_after_opening_and_before_closing_nonempty_brackets_option(
                context.options.clone(),
            )
            .is_true()
        }
        "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingNonemptyParenthesisOption)" => {
            insert_space_after_opening_and_before_closing_nonempty_parenthesis_option(
                context.options.clone(),
            )
            .is_true()
        }
        "isOptionEnabled(insertSpaceAfterOpeningAndBeforeClosingTemplateStringBracesOption)" => {
            insert_space_after_opening_and_before_closing_template_string_braces_option(
                context.options.clone(),
            )
            .is_true()
        }
        "isOptionEnabled(insertSpaceAfterSemicolonInForStatementsOption)" => {
            insert_space_after_semicolon_in_for_statements_option(context.options.clone()).is_true()
        }
        "isOptionEnabled(insertSpaceAfterTypeAssertionOption)" => {
            insert_space_after_type_assertion_option(context.options.clone()).is_true()
        }
        "isOptionEnabled(insertSpaceBeforeAndAfterBinaryOperatorsOption)" => {
            insert_space_before_and_after_binary_operators_option(context.options.clone()).is_true()
        }
        "isOptionEnabled(insertSpaceBeforeFunctionParenthesisOption)" => {
            insert_space_before_function_parenthesis_option(context.options.clone()).is_true()
        }
        "isOptionEnabled(insertSpaceBeforeTypeAnnotationOption)" => {
            insert_space_before_type_annotation_option(context.options.clone()).is_true()
        }
        "isOptionEnabled(placeOpenBraceOnNewLineForControlBlocksOption)" => {
            place_open_brace_on_new_line_for_control_blocks_option(context.options.clone())
                .is_true()
        }
        "isOptionEnabled(placeOpenBraceOnNewLineForFunctionsOption)" => {
            place_open_brace_on_new_line_for_functions_option(context.options.clone()).is_true()
        }
        "isOptionEnabledOrUndefined(insertSpaceAfterOpeningAndBeforeClosingNonemptyBracesOption)" => {
            insert_space_after_opening_and_before_closing_nonempty_braces_option(
                context.options.clone(),
            )
            .is_true_or_unknown()
        }
        "optionEquals(semicolonOption, lsutil.SemicolonPreferenceInsert)" => {
            semicolon_option(context.options.clone()) == lsutil::SemicolonPreference::Insert
        }
        "optionEquals(semicolonOption, lsutil.SemicolonPreferenceRemove)" => {
            semicolon_option(context.options.clone()) == lsutil::SemicolonPreference::Remove
        }
        _ => panic!("unknown rule context predicate: {name}"),
    }
}

pub fn get_rule_bucket_index(row: ast::Kind, column: ast::Kind) -> usize {
    debug_assert!(
        row <= ast::Kind::LastKeyword && column <= ast::Kind::LastKeyword,
        "Must compute formatting context from tokens"
    );
    (row as usize * MAP_ROW_LENGTH) + column as usize
}

const MASK_BIT_SIZE: usize = 5;
const MASK: usize = 0b11111; // MaskBitSize bits
const MAP_ROW_LENGTH: usize = ast::Kind::LastToken as usize + 1;

/**
 * For a given rule action, gets a mask of other rule actions that
 * cannot be applied at the same position.
 */
pub fn get_rule_action_exclusion(rule_action: RuleAction) -> RuleAction {
    let mut mask = RuleAction::NONE;
    if rule_action.0 & RuleAction::STOP_PROCESSING_SPACE_ACTIONS.0 != 0 {
        mask = RuleAction(mask.0 | RuleAction::MODIFY_SPACE_ACTION.0);
    }
    if rule_action.0 & RuleAction::STOP_PROCESSING_TOKEN_ACTIONS.0 != 0 {
        mask = RuleAction(mask.0 | RuleAction::MODIFY_TOKEN_ACTION.0);
    }
    if rule_action.0 & RuleAction::MODIFY_SPACE_ACTION.0 != 0 {
        mask = RuleAction(mask.0 | RuleAction::MODIFY_SPACE_ACTION.0);
    }
    if rule_action.0 & RuleAction::MODIFY_TOKEN_ACTION.0 != 0 {
        mask = RuleAction(mask.0 | RuleAction::MODIFY_TOKEN_ACTION.0);
    }
    mask
}

static GET_RULES_MAP: OnceLock<Vec<Vec<RuleImpl>>> = OnceLock::new();

pub fn get_rules_map() -> &'static Vec<Vec<RuleImpl>> {
    GET_RULES_MAP.get_or_init(build_rules_map)
}

pub fn build_rules_map() -> Vec<Vec<RuleImpl>> {
    let rules = get_all_rules();
    // Map from bucket index to array of rules
    let mut m = vec![Vec::new(); MAP_ROW_LENGTH * MAP_ROW_LENGTH];
    // This array is used only during construction of the rulesbucket in the map
    let mut rules_bucket_construction_state_list = vec![0usize; m.len()];
    for rule in rules {
        let specific_rule = rule.left_token_range.is_specific && rule.right_token_range.is_specific;

        for left in rule.left_token_range.tokens {
            for right in rule.right_token_range.tokens.iter().copied() {
                let index = get_rule_bucket_index(left, right);
                m[index] = add_rule(
                    m[index].clone(),
                    rule.rule.clone(),
                    specific_rule,
                    &mut rules_bucket_construction_state_list,
                    index,
                );
            }
        }
    }
    m
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RulesPosition(pub usize);

pub const RULES_POSITION_STOP_RULES_SPECIFIC: RulesPosition = RulesPosition(0);
pub const RULES_POSITION_STOP_RULES_ANY: RulesPosition = RulesPosition(MASK_BIT_SIZE);
pub const RULES_POSITION_CONTEXT_RULES_SPECIFIC: RulesPosition = RulesPosition(MASK_BIT_SIZE * 2);
pub const RULES_POSITION_CONTEXT_RULES_ANY: RulesPosition = RulesPosition(MASK_BIT_SIZE * 3);
pub const RULES_POSITION_NO_CONTEXT_RULES_SPECIFIC: RulesPosition =
    RulesPosition(MASK_BIT_SIZE * 4);
pub const RULES_POSITION_NO_CONTEXT_RULES_ANY: RulesPosition = RulesPosition(MASK_BIT_SIZE * 5);

// The Rules list contains all the inserted rules into a rulebucket in the following order:
//
//  1- Ignore rules with specific token combination
//  2- Ignore rules with any token combination
//  3- Context rules with specific token combination
//  4- Context rules with any token combination
//  5- Non-context rules with specific token combination
//  6- Non-context rules with any token combination
//
// The member rulesInsertionIndexBitmap is used to describe the number of rules
// in each sub-bucket (above) hence can be used to know the index of where to insert
// the next rule. It's a bitmap which contains 6 different sections each is given 5 bits.
//
// Example:
// In order to insert a rule to the end of sub-bucket (3), we get the index by adding
// the values in the bitmap segments 3rd, 2nd, and 1st.
pub fn add_rule(
    mut rules: Vec<RuleImpl>,
    rule: RuleImpl,
    specific_tokens: bool,
    construction_state: &mut [usize],
    rules_bucket_index: usize,
) -> Vec<RuleImpl> {
    let position;
    if rule.action().0 & RuleAction::STOP_ACTION.0 != 0 {
        if specific_tokens {
            position = RULES_POSITION_STOP_RULES_SPECIFIC;
        } else {
            position = RULES_POSITION_STOP_RULES_ANY;
        }
    } else if !rule.context().is_empty() || !rule.context_names().is_empty() {
        if specific_tokens {
            position = RULES_POSITION_CONTEXT_RULES_SPECIFIC;
        } else {
            position = RULES_POSITION_CONTEXT_RULES_ANY;
        }
    } else if specific_tokens {
        position = RULES_POSITION_NO_CONTEXT_RULES_SPECIFIC;
    } else {
        position = RULES_POSITION_NO_CONTEXT_RULES_ANY;
    }

    let state = construction_state[rules_bucket_index];

    rules.insert(get_rule_insertion_index(state, position), rule);
    construction_state[rules_bucket_index] = increase_insertion_index(state, position);
    rules
}

pub fn get_rule_insertion_index(mut index_bitmap: usize, mask_position: RulesPosition) -> usize {
    let mut index = 0;
    let mut pos = 0;
    while pos <= mask_position.0 {
        index += index_bitmap & MASK;
        index_bitmap >>= MASK_BIT_SIZE;
        pos += MASK_BIT_SIZE;
    }
    index
}

pub fn increase_insertion_index(index_bitmap: usize, mask_position: RulesPosition) -> usize {
    let value = ((index_bitmap >> mask_position.0) & MASK) + 1;
    debug_assert!(
        (value & MASK) == value,
        "Adding more rules into the sub-bucket than allowed. Maximum allowed is 32 rules."
    );
    (index_bitmap & !(MASK << mask_position.0)) | (value << mask_position.0)
}
