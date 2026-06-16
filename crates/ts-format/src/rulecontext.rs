use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_scanner as scanner;

use crate::{FORMAT_REQUEST_KIND_FORMAT_ON_ENTER, FormattingContext, TextRangeWithKind, lsutil};

///
/// Contexts
///

pub type OptionSelector = fn(lsutil::FormatCodeSettings) -> core::Tristate;
pub type AnyOptionSelector<T> = fn(lsutil::FormatCodeSettings) -> T;
pub type ContextPredicate = Box<dyn Fn(&mut FormattingContext) -> bool>;

pub fn semicolon_option(options: lsutil::FormatCodeSettings) -> lsutil::SemicolonPreference {
    options.semicolons
}

pub fn insert_space_after_comma_delimiter_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_comma_delimiter
}

pub fn insert_space_after_semicolon_in_for_statements_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_semicolon_in_for_statements
}

pub fn insert_space_before_and_after_binary_operators_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_before_and_after_binary_operators
}

pub fn insert_space_after_constructor_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_constructor
}

pub fn insert_space_after_keywords_in_control_flow_statements_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_keywords_in_control_flow_statements
}

pub fn insert_space_after_function_keyword_for_anonymous_functions_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_function_keyword_for_anonymous_functions
}

pub fn insert_space_after_opening_and_before_closing_nonempty_parenthesis_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_opening_and_before_closing_nonempty_parenthesis
}

pub fn insert_space_after_opening_and_before_closing_nonempty_brackets_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_opening_and_before_closing_nonempty_brackets
}

pub fn insert_space_after_opening_and_before_closing_nonempty_braces_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_opening_and_before_closing_nonempty_braces
}

pub fn insert_space_after_opening_and_before_closing_empty_braces_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_opening_and_before_closing_empty_braces
}

pub fn insert_space_after_opening_and_before_closing_template_string_braces_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_opening_and_before_closing_template_string_braces
}

pub fn insert_space_after_opening_and_before_closing_jsx_expression_braces_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_opening_and_before_closing_jsx_expression_braces
}

pub fn insert_space_after_type_assertion_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_after_type_assertion
}

pub fn insert_space_before_function_parenthesis_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_before_function_parenthesis
}

pub fn place_open_brace_on_new_line_for_functions_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.place_open_brace_on_new_line_for_functions
}

pub fn place_open_brace_on_new_line_for_control_blocks_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.place_open_brace_on_new_line_for_control_blocks
}

pub fn insert_space_before_type_annotation_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.insert_space_before_type_annotation
}

pub fn indent_multi_line_object_literal_beginning_on_blank_line_option(
    options: lsutil::FormatCodeSettings,
) -> core::Tristate {
    options.indent_multi_line_object_literal_beginning_on_blank_line
}

pub fn indent_switch_case_option(options: lsutil::FormatCodeSettings) -> core::Tristate {
    options.indent_switch_case
}

pub fn option_equals<T: Copy + PartialEq + 'static>(
    option_name: AnyOptionSelector<T>,
    option_value: T,
) -> ContextPredicate {
    Box::new(move |context| option_name(context.options.clone()) == option_value)
}

pub fn is_option_enabled(option_name: OptionSelector) -> ContextPredicate {
    Box::new(move |context| option_name(context.options.clone()).is_true())
}

pub fn is_option_disabled(option_name: OptionSelector) -> ContextPredicate {
    Box::new(move |context| option_name(context.options.clone()).is_false())
}

pub fn is_option_disabled_or_undefined(option_name: OptionSelector) -> ContextPredicate {
    Box::new(move |context| option_name(context.options.clone()).is_false_or_unknown())
}

pub fn is_option_disabled_or_undefined_or_tokens_on_same_line(
    option_name: OptionSelector,
) -> ContextPredicate {
    Box::new(move |context| {
        option_name(context.options.clone()).is_false_or_unknown()
            || context.tokens_are_on_same_line()
    })
}

pub fn is_option_enabled_or_undefined(option_name: OptionSelector) -> ContextPredicate {
    Box::new(move |context| option_name(context.options.clone()).is_true_or_unknown())
}

fn kind(store: &ast::AstStore, node: Option<ast::Node>) -> Option<ast::Kind> {
    node.map(|node| store.kind(node))
}

fn context_node_kind(context: &FormattingContext) -> Option<ast::Kind> {
    kind(context.source_file.store(), context.context_node)
}

fn current_token_parent_kind(context: &FormattingContext) -> Option<ast::Kind> {
    kind(context.source_file.store(), context.current_token_parent)
}

fn next_token_parent_kind(context: &FormattingContext) -> Option<ast::Kind> {
    kind(context.source_file.store(), context.next_token_parent)
}

pub fn is_for_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ForStatement)
}

pub fn is_not_for_context(context: &mut FormattingContext) -> bool {
    !is_for_context(context)
}

pub fn is_binary_op_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    let Some(context_node) = context.context_node else {
        return false;
    };
    match store.kind(context_node) {
        ast::Kind::BinaryExpression => {
            store
                .operator_token(context_node)
                .is_none_or(|operator_token| store.kind(operator_token) != ast::Kind::CommaToken)
        }
        ast::Kind::ConditionalExpression
        | ast::Kind::ConditionalType
        | ast::Kind::AsExpression
        | ast::Kind::ExportSpecifier
        | ast::Kind::ImportSpecifier
        | ast::Kind::TypePredicate
        | ast::Kind::UnionType
        | ast::Kind::IntersectionType
        | ast::Kind::SatisfiesExpression => true,

        // equals in binding elements func foo([[x, y] = [1, 2]])
        ast::Kind::BindingElement
        // equals in type X = ...
        | ast::Kind::TypeAliasDeclaration
        // equal in import a = module('a');
        | ast::Kind::ImportEqualsDeclaration
        // equal in export = 1
        | ast::Kind::ExportAssignment
        // equal in let a = 0
        | ast::Kind::VariableDeclaration
        // equal in p = 0
        | ast::Kind::Parameter
        | ast::Kind::EnumMember
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature => {
            context.current_token_span.kind == ast::Kind::EqualsToken
                || context.next_token_span.kind == ast::Kind::EqualsToken
        }
        // "in" keyword in for (let x in []) { }
        ast::Kind::ForInStatement
        // "in" keyword in [P in keyof T] T[P]
        | ast::Kind::TypeParameter => {
            context.current_token_span.kind == ast::Kind::InKeyword
                || context.next_token_span.kind == ast::Kind::InKeyword
                || context.current_token_span.kind == ast::Kind::EqualsToken
                || context.next_token_span.kind == ast::Kind::EqualsToken
        }
        // Technically, "of" is not a binary operator, but format it the same way as "in"
        ast::Kind::ForOfStatement => {
            context.current_token_span.kind == ast::Kind::OfKeyword
                || context.next_token_span.kind == ast::Kind::OfKeyword
        }
        _ => false,
    }
}

pub fn is_not_binary_op_context(context: &mut FormattingContext) -> bool {
    !is_binary_op_context(context)
}

pub fn is_not_type_annotation_context(context: &mut FormattingContext) -> bool {
    !is_type_annotation_context(context)
}

pub fn is_type_annotation_context(context: &mut FormattingContext) -> bool {
    let Some(context_kind) = context_node_kind(context) else {
        return false;
    };
    context_kind == ast::Kind::PropertyDeclaration
        || context_kind == ast::Kind::PropertySignature
        || context_kind == ast::Kind::Parameter
        || context_kind == ast::Kind::VariableDeclaration
        || ast::is_function_like_kind(context_kind)
}

pub fn is_optional_property_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    context.context_node.is_some_and(|node| {
        ast::is_property_declaration(store, node) && ast::has_question_token(store, node)
    })
}

pub fn is_non_optional_property_context(context: &mut FormattingContext) -> bool {
    !is_optional_property_context(context)
}

pub fn is_conditional_operator_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ConditionalExpression)
        || context_node_kind(context) == Some(ast::Kind::ConditionalType)
}

pub fn is_same_line_token_or_before_block_context(context: &mut FormattingContext) -> bool {
    context.tokens_are_on_same_line() || is_before_block_context(context)
}

pub fn is_brace_wrapped_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ObjectBindingPattern)
        || context_node_kind(context) == Some(ast::Kind::MappedType)
        || is_single_line_block_context(context)
}

// This check is done before an open brace in a control construct, a function, or a typescript block declaration
pub fn is_before_multiline_block_context(context: &mut FormattingContext) -> bool {
    is_before_block_context(context)
        && !(context.next_node_all_on_same_line() || context.next_node_block_is_on_one_line())
}

pub fn is_multiline_block_context(context: &mut FormattingContext) -> bool {
    is_block_context(context)
        && !(context.context_node_all_on_same_line() || context.context_node_block_is_on_one_line())
}

pub fn is_single_line_block_context(context: &mut FormattingContext) -> bool {
    is_block_context(context)
        && (context.context_node_all_on_same_line() || context.context_node_block_is_on_one_line())
}

pub fn is_block_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    context
        .context_node
        .is_some_and(|node| node_is_block_context(store, node))
}

pub fn is_before_block_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    context
        .next_token_parent
        .is_some_and(|node| node_is_block_context(store, node))
}

// IMPORTANT!!! This method must return true ONLY for nodes with open and close braces as immediate children
pub fn node_is_block_context(store: &ast::AstStore, node: ast::Node) -> bool {
    if node_is_type_script_decl_with_block_context(store, node) {
        // This means we are in a context that looks like a block to the user, but in the grammar is actually not a node (it's a class, module, enum, object type literal, etc).
        return true;
    }

    match store.kind(node) {
        ast::Kind::Block
        | ast::Kind::CaseBlock
        | ast::Kind::ObjectLiteralExpression
        | ast::Kind::ModuleBlock => true,
        _ => false,
    }
}

pub fn is_function_decl_context(context: &mut FormattingContext) -> bool {
    match context_node_kind(context) {
        Some(
            ast::Kind::FunctionDeclaration
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            // case ast.KindMemberFunctionDeclaration:
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            // case ast.KindMethodSignature:
            | ast::Kind::CallSignature
            | ast::Kind::FunctionExpression
            | ast::Kind::Constructor
            | ast::Kind::ArrowFunction
            // case ast.KindConstructorDeclaration:
            // case ast.KindSimpleArrowFunctionExpression:
            // case ast.KindParenthesizedArrowFunctionExpression:
            | ast::Kind::InterfaceDeclaration,
        ) => true, // This one is not truly a function, but for formatting purposes, it acts just like one
        _ => false,
    }
}

pub fn is_not_function_decl_context(context: &mut FormattingContext) -> bool {
    !is_function_decl_context(context)
}

pub fn is_function_declaration_or_function_expression_context(
    context: &mut FormattingContext,
) -> bool {
    context_node_kind(context) == Some(ast::Kind::FunctionDeclaration)
        || context_node_kind(context) == Some(ast::Kind::FunctionExpression)
}

pub fn is_type_script_decl_with_block_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    context
        .context_node
        .is_some_and(|node| node_is_type_script_decl_with_block_context(store, node))
}

pub fn node_is_type_script_decl_with_block_context(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::TypeLiteral
        | ast::Kind::ModuleDeclaration
        | ast::Kind::ExportDeclaration
        | ast::Kind::NamedExports
        | ast::Kind::ImportDeclaration
        | ast::Kind::NamedImports => true,
        _ => false,
    }
}

pub fn is_after_code_block_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    let Some(current_token_parent) = context.current_token_parent else {
        return false;
    };
    match store.kind(current_token_parent) {
        ast::Kind::ClassDeclaration
        | ast::Kind::ModuleDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::CatchClause
        | ast::Kind::ModuleBlock
        | ast::Kind::SwitchStatement => true,
        ast::Kind::Block => {
            let block_parent = store.parent(current_token_parent);
            // In a codefix scenario, we can't rely on parents being set. So just always return true.
            block_parent.is_none_or(|block_parent| {
                store.kind(block_parent) != ast::Kind::ArrowFunction
                    && store.kind(block_parent) != ast::Kind::FunctionExpression
            })
        }
        _ => false,
    }
}

pub fn is_control_decl_context(context: &mut FormattingContext) -> bool {
    match context_node_kind(context) {
        Some(
            ast::Kind::IfStatement
            | ast::Kind::SwitchStatement
            | ast::Kind::ForStatement
            | ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement
            | ast::Kind::WhileStatement
            | ast::Kind::TryStatement
            | ast::Kind::DoStatement
            | ast::Kind::WithStatement
            // Go source notes a possible future ast.KindElseClause case; this AST has no ElseClause kind.
            | ast::Kind::CatchClause,
        ) => true,
        _ => false,
    }
}

pub fn is_object_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ObjectLiteralExpression)
}

pub fn is_function_call_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::CallExpression)
}

pub fn is_new_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::NewExpression)
}

pub fn is_function_call_or_new_context(context: &mut FormattingContext) -> bool {
    is_function_call_context(context) || is_new_context(context)
}

pub fn is_previous_token_not_comma(context: &mut FormattingContext) -> bool {
    context.current_token_span.kind != ast::Kind::CommaToken
}

pub fn is_next_token_not_close_bracket(context: &mut FormattingContext) -> bool {
    context.next_token_span.kind != ast::Kind::CloseBracketToken
}

pub fn is_next_token_not_close_paren(context: &mut FormattingContext) -> bool {
    context.next_token_span.kind != ast::Kind::CloseParenToken
}

pub fn is_arrow_function_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ArrowFunction)
}

pub fn is_import_type_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ImportType)
}

pub fn is_non_jsx_same_line_token_context(context: &mut FormattingContext) -> bool {
    context.tokens_are_on_same_line() && context_node_kind(context) != Some(ast::Kind::JsxText)
}

pub fn is_non_jsx_text_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) != Some(ast::Kind::JsxText)
}

pub fn is_non_jsx_element_or_fragment_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) != Some(ast::Kind::JsxElement)
        && context_node_kind(context) != Some(ast::Kind::JsxFragment)
}

pub fn is_jsx_expression_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::JsxExpression)
        || context_node_kind(context) == Some(ast::Kind::JsxSpreadAttribute)
}

pub fn is_next_token_parent_jsx_attribute(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    let Some(next_token_parent) = context.next_token_parent else {
        return false;
    };
    store.kind(next_token_parent) == ast::Kind::JsxAttribute
        || (store.kind(next_token_parent) == ast::Kind::JsxNamespacedName
            && store
                .parent(next_token_parent)
                .is_some_and(|parent| store.kind(parent) == ast::Kind::JsxAttribute))
}

pub fn is_jsx_attribute_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::JsxAttribute)
}

pub fn is_next_token_parent_not_jsx_namespaced_name(context: &mut FormattingContext) -> bool {
    next_token_parent_kind(context) != Some(ast::Kind::JsxNamespacedName)
}

pub fn is_next_token_parent_jsx_namespaced_name(context: &mut FormattingContext) -> bool {
    next_token_parent_kind(context) == Some(ast::Kind::JsxNamespacedName)
}

pub fn is_jsx_self_closing_element_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::JsxSelfClosingElement)
}

pub fn is_not_before_block_in_function_declaration_context(
    context: &mut FormattingContext,
) -> bool {
    !is_function_decl_context(context) && !is_before_block_context(context)
}

pub fn is_end_of_decorator_context_on_same_line(context: &mut FormattingContext) -> bool {
    if !context.tokens_are_on_same_line() {
        return false;
    }
    let store = context.source_file.store();
    context
        .context_node
        .is_some_and(|node| ast::has_decorators(store, node))
        && context
            .current_token_parent
            .is_some_and(|node| node_is_in_decorator_context(store, node))
        && !context
            .next_token_parent
            .is_some_and(|node| node_is_in_decorator_context(store, node))
}

pub fn node_is_in_decorator_context(store: &ast::AstStore, node: ast::Node) -> bool {
    let mut current = Some(node);
    while current.is_some_and(|node| ast::is_expression(store, node)) {
        current = current.and_then(|node| store.parent(node));
    }
    current.is_some_and(|node| store.kind(node) == ast::Kind::Decorator)
}

pub fn is_start_of_variable_declaration_list(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    context
        .current_token_parent
        .as_ref()
        .is_some_and(|current_token_parent| {
            store.kind(*current_token_parent) == ast::Kind::VariableDeclarationList
                && scanner::get_token_pos_of_node(current_token_parent, &context.source_file, false)
                    == context.current_token_span.loc.pos() as usize
        })
}

pub fn is_not_format_on_enter(context: &mut FormattingContext) -> bool {
    context.formatting_request_kind != FORMAT_REQUEST_KIND_FORMAT_ON_ENTER
}

pub fn is_module_decl_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ModuleDeclaration)
}

pub fn is_object_type_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::TypeLiteral) // && context.contextNode.parent.Kind != ast.KindInterfaceDeclaration;
}

pub fn is_constructor_signature_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::ConstructSignature)
}

pub fn is_type_argument_or_parameter_or_assertion(
    store: &ast::AstStore,
    token: TextRangeWithKind,
    parent: Option<ast::Node>,
) -> bool {
    if token.kind != ast::Kind::LessThanToken && token.kind != ast::Kind::GreaterThanToken {
        return false;
    }
    match kind(store, parent) {
        Some(
            ast::Kind::TypeReference
            | ast::Kind::TypeAssertionExpression
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::ClassDeclaration
            | ast::Kind::ClassExpression
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::CallSignature
            | ast::Kind::ConstructSignature
            | ast::Kind::CallExpression
            | ast::Kind::NewExpression
            | ast::Kind::ExpressionWithTypeArguments,
        ) => true,
        _ => false,
    }
}

pub fn is_type_argument_or_parameter_or_assertion_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    is_type_argument_or_parameter_or_assertion(
        store,
        context.current_token_span.clone(),
        context.current_token_parent,
    ) || is_type_argument_or_parameter_or_assertion(
        store,
        context.next_token_span.clone(),
        context.next_token_parent,
    )
}

pub fn is_type_assertion_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::TypeAssertionExpression)
}

pub fn is_non_type_assertion_context(context: &mut FormattingContext) -> bool {
    !is_type_assertion_context(context)
}

pub fn is_void_op_context(context: &mut FormattingContext) -> bool {
    context.current_token_span.kind == ast::Kind::VoidKeyword
        && current_token_parent_kind(context) == Some(ast::Kind::VoidExpression)
}

pub fn is_yield_or_yield_star_with_operand(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    context.context_node.is_some_and(|node| {
        store.kind(node) == ast::Kind::YieldExpression && store.expression(node).is_some()
    })
}

pub fn is_non_null_assertion_context(context: &mut FormattingContext) -> bool {
    context_node_kind(context) == Some(ast::Kind::NonNullExpression)
}

pub fn is_not_statement_condition_context(context: &mut FormattingContext) -> bool {
    !is_statement_condition_context(context)
}

pub fn is_statement_condition_context(context: &mut FormattingContext) -> bool {
    match context_node_kind(context) {
        Some(
            ast::Kind::IfStatement
            | ast::Kind::ForStatement
            | ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement
            | ast::Kind::DoStatement
            | ast::Kind::WhileStatement,
        ) => true,
        _ => false,
    }
}

pub fn is_semicolon_deletion_context(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    let mut next_token_kind = context.next_token_span.kind;
    let mut next_token_start = context.next_token_span.loc.pos();
    if ast::is_trivia(next_token_kind) {
        let next_real_token = if context.next_token_parent == context.current_token_parent {
            // Matches Go's inherited Strada note: find the next token from the shared parent.
            context
                .next_token_parent
                .as_ref()
                .and_then(|next_token_parent| {
                    astnav::find_next_token(
                        *next_token_parent,
                        *next_token_parent,
                        &context.source_file,
                    )
                })
        } else {
            context
                .next_token_parent
                .as_ref()
                .and_then(|next_token_parent| {
                    lsutil::get_first_token(Some(*next_token_parent), &context.source_file)
                })
        };

        let Some(next_real_token) = next_real_token else {
            return true;
        };
        next_token_kind = store.kind(next_real_token);
        next_token_start =
            scanner::get_token_pos_of_node(&next_real_token, &context.source_file, false) as i32;
    }

    let start_line = scanner::get_ecma_line_of_position(
        &context.source_file,
        context.current_token_span.loc.pos(),
    );
    let end_line = scanner::get_ecma_line_of_position(&context.source_file, next_token_start);
    if start_line == end_line {
        return next_token_kind == ast::Kind::CloseBraceToken
            || next_token_kind == ast::Kind::EndOfFile;
    }

    if next_token_kind == ast::Kind::SemicolonToken
        && context.current_token_span.kind == ast::Kind::SemicolonToken
    {
        return true;
    }

    if next_token_kind == ast::Kind::SemicolonClassElement
        || next_token_kind == ast::Kind::SemicolonToken
    {
        return false;
    }

    if context_node_kind(context) == Some(ast::Kind::InterfaceDeclaration)
        || context_node_kind(context) == Some(ast::Kind::TypeAliasDeclaration)
    {
        // Can't remove semicolon after `foo`; it would parse as a method declaration:
        //
        // interface I {
        //   foo;
        //   () void
        // }
        return current_token_parent_kind(context) != Some(ast::Kind::PropertySignature)
            || context
                .current_token_parent
                .is_none_or(|current_token_parent| store.r#type(current_token_parent).is_some())
            || next_token_kind != ast::Kind::OpenParenToken;
    }

    if context
        .current_token_parent
        .is_some_and(|current_token_parent| {
            ast::is_property_declaration(store, current_token_parent)
        })
    {
        return context
            .current_token_parent
            .is_none_or(|current_token_parent| store.initializer(current_token_parent).is_none());
    }

    current_token_parent_kind(context) != Some(ast::Kind::ForStatement)
        && current_token_parent_kind(context) != Some(ast::Kind::EmptyStatement)
        && current_token_parent_kind(context) != Some(ast::Kind::SemicolonClassElement)
        && next_token_kind != ast::Kind::OpenBracketToken
        && next_token_kind != ast::Kind::OpenParenToken
        && next_token_kind != ast::Kind::PlusToken
        && next_token_kind != ast::Kind::MinusToken
        && next_token_kind != ast::Kind::SlashToken
        && next_token_kind != ast::Kind::RegularExpressionLiteral
        && next_token_kind != ast::Kind::CommaToken
        && next_token_kind != ast::Kind::TemplateExpression
        && next_token_kind != ast::Kind::TemplateHead
        && next_token_kind != ast::Kind::NoSubstitutionTemplateLiteral
        && next_token_kind != ast::Kind::DotToken
}

pub fn is_semicolon_insertion_context(context: &mut FormattingContext) -> bool {
    if let Some(current_token_parent) = context.current_token_parent.as_ref() {
        lsutil::position_is_asi_candidate(
            context.current_token_span.loc.end(),
            current_token_parent,
            &context.source_file,
        )
    } else {
        let source_file_node = context.source_file.as_node();
        lsutil::position_is_asi_candidate(
            context.current_token_span.loc.end(),
            &source_file_node,
            &context.source_file,
        )
    }
}

pub fn is_not_property_access_on_integer_literal(context: &mut FormattingContext) -> bool {
    let store = context.source_file.store();
    !context
        .context_node
        .is_some_and(|node| ast::is_property_access_expression(store, node))
        || !context
            .context_node
            .and_then(|node| store.expression(node))
            .is_some_and(|node| ast::is_numeric_literal(store, node))
        || context
            .context_node
            .and_then(|node| store.expression(node))
            .is_some_and(|expression| store.text(expression).contains('.'))
}
