use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::{
    get_line_start_position_for_position, range_is_on_one_line,
    span::is_string_or_regular_expression_or_template_literal,
};

pub type NextTokenKind = i32;
pub const NEXT_TOKEN_KIND_UNKNOWN: NextTokenKind = 0;
pub const NEXT_TOKEN_KIND_OPEN_BRACE: NextTokenKind = 1;
pub const NEXT_TOKEN_KIND_CLOSE_BRACE: NextTokenKind = 2;

pub fn get_indentation_for_node(
    n: &ast::Node,
    ignore_actual_indentation_range: Option<&core::TextRange>,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    let (start_line, start_pos) = scanner::get_ecma_line_and_byte_offset_of_position(
        source_file,
        scanner::get_token_pos_of_node(n, source_file, false),
    );
    get_indentation_for_node_worker(
        n,
        start_line as i32,
        start_pos as i32,
        ignore_actual_indentation_range,
        0,
        source_file,
        false,
        options,
    )
}

// GetIndentation computes the expected indentation for a position in a source file.
// This is the Go port of SmartIndenter.getIndentation from TypeScript.
pub fn get_indentation(
    position: i32,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
    assume_new_line_before_close_brace: bool,
) -> i32 {
    if position as usize > source_file.text().len() {
        return options.base_indent_size; // past EOF
    }

    // no indentation when the indent style is set to none,
    // so we can return fast
    if options.indent_style == crate::lsutil::IndentStyle::None {
        return 0;
    }

    let store = source_file.store();
    let preceding_token_info = astnav::find_preceding_token_ex_info(source_file, position, None);
    let preceding_token = preceding_token_info.and_then(|info| info.node);

    let enclosing_comment_range =
        get_range_of_enclosing_comment(source_file, position, preceding_token.as_ref());
    if enclosing_comment_range
        .as_ref()
        .is_some_and(|range| range.kind == ast::Kind::MultiLineCommentTrivia)
    {
        return get_comment_indent(
            source_file,
            position,
            options,
            enclosing_comment_range.as_ref().unwrap(),
        );
    }

    let Some(preceding_token) = preceding_token else {
        if preceding_token_info.is_some_and(|info| {
            info.kind == ast::Kind::CloseBraceToken && info.loc.end() <= position
        }) {
            return get_block_indent(source_file, position, options);
        }
        return options.base_indent_size;
    };

    // no indentation in string/regex/template literals
    if is_string_or_regular_expression_or_template_literal(store.kind(preceding_token)) {
        let token_start = scanner::get_token_pos_of_node(&preceding_token, source_file, false);
        if token_start as i32 <= position && position < store.loc(preceding_token).end() {
            return 0;
        }
    }

    let line_at_position = scanner::get_ecma_line_of_position(source_file, position) as i32;

    // indentation is first non-whitespace character in a previous line
    // for block indentation, we should look for a line which contains something that's not
    // whitespace.
    let Some(current_token) = astnav::get_token_at_position(source_file, position) else {
        return options.base_indent_size;
    };
    // For object literals, we want indentation to work just like with blocks.
    // If the `{` starts in any position (even in the middle of a line), then
    // the following indentation should treat `{` as the start of that line (including leading whitespace).
    // ```
    //     const a: { x: undefined, y: undefined } = {}       // leading 4 whitespaces and { starts in the middle of line
    // ->
    //     const a: { x: undefined, y: undefined } = {
    //         x: undefined,
    //         y: undefined,
    //     }
    // ---------------------
    //     const a: {x : undefined, y: undefined } =
    //      {}
    // ->
    //     const a: { x: undefined, y: undefined } =
    //      {                                                  // leading 5 whitespaces and { starts at 6 column
    //          x: undefined,
    //          y: undefined,
    //      }
    // ```
    let is_object_literal = store.kind(current_token) == ast::Kind::OpenBraceToken
        && store
            .parent(current_token)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::ObjectLiteralExpression);
    if options.indent_style == crate::lsutil::IndentStyle::Block || is_object_literal {
        return get_block_indent(source_file, position, options);
    }

    if store.kind(preceding_token) == ast::Kind::CommaToken
        && store
            .parent(preceding_token)
            .is_some_and(|parent| store.kind(parent) != ast::Kind::BinaryExpression)
    {
        // previous token is comma that separates items in list - find the previous item and try to derive indentation from it
        let actual_indentation = get_actual_indentation_for_list_item_before_comma(
            &preceding_token,
            source_file,
            options.clone(),
        );
        if actual_indentation != -1 {
            return actual_indentation;
        }
    }

    let preceding_token_parent = store.parent(preceding_token);
    let container_list =
        get_list_by_position(position, preceding_token_parent.as_ref(), source_file);
    // use list position if the preceding token is before any list items
    if let Some(container_list) = container_list {
        if !store
            .loc(preceding_token)
            .contained_by(container_list.loc())
        {
            let use_the_same_base_indentation = store.parent(current_token).is_some_and(|parent| {
                store.kind(parent) == ast::Kind::FunctionExpression
                    || store.kind(parent) == ast::Kind::ArrowFunction
            });
            let mut indent_size = 0;
            if !use_the_same_base_indentation {
                indent_size = options.indent_size;
            }
            let res = get_actual_indentation_for_list_start_line(
                &container_list,
                source_file,
                options.clone(),
            );
            if res == -1 {
                return indent_size;
            }
            return res + indent_size;
        }
    }

    get_smart_indent(
        source_file,
        position,
        &preceding_token,
        line_at_position,
        assume_new_line_before_close_brace,
        options,
    )
}

pub fn get_comment_indent(
    source_file: &ast::SourceFile,
    position: i32,
    options: crate::lsutil::FormatCodeSettings,
    enclosing_comment_range: &ast::CommentRange,
) -> i32 {
    let previous_line = scanner::get_ecma_line_of_position(source_file, position) as i32 - 1;
    let comment_start_line =
        scanner::get_ecma_line_of_position(source_file, enclosing_comment_range.pos()) as i32;

    assert!(comment_start_line >= 0, "commentStartLine >= 0");

    if previous_line <= comment_start_line {
        let line_starts = scanner::get_ecma_line_starts(source_file);
        return find_first_non_whitespace_column(
            line_starts[comment_start_line as usize] as i32,
            position,
            source_file,
            options,
        );
    }

    let line_starts = scanner::get_ecma_line_starts(source_file);
    let start_position_of_line = line_starts[previous_line as usize] as i32;
    let (character, column) = find_first_non_whitespace_character_and_column(
        start_position_of_line,
        position,
        source_file,
        options,
    );

    if column == 0 {
        return column;
    }

    let first_non_whitespace_character_code =
        source_file.text().as_bytes()[(start_position_of_line + character) as usize] as char;
    if first_non_whitespace_character_code == '*' {
        return column - 1;
    }
    column
}

pub fn get_leading_comment_ranges_of_node(
    node: &ast::Node,
    file: &ast::SourceFile,
) -> Vec<ast::CommentRange> {
    if file.store().kind(*node) == ast::Kind::JsxText {
        return Vec::new();
    }
    scanner::get_leading_comment_ranges(file.text(), file.store().loc(*node).pos())
}

pub fn get_range_of_enclosing_comment(
    source_file: &ast::SourceFile,
    position: i32,
    preceding_token: Option<&ast::Node>,
) -> Option<ast::CommentRange> {
    let Some(token_at_position) = astnav::get_token_at_position(source_file, position) else {
        return None;
    };
    let store = source_file.store();
    let token_start = astnav::get_start_of_node(token_at_position, source_file);
    if token_start <= position && position < store.loc(token_at_position).end() {
        return None;
    }

    // Between two consecutive tokens, all comments are either trailing on the former
    // or leading on the latter (and none are in both lists).
    let mut comment_ranges = Vec::new();
    if let Some(preceding_token) = preceding_token {
        comment_ranges.extend(scanner::get_trailing_comment_ranges(
            source_file.text(),
            store.loc(*preceding_token).end(),
        ));
    }
    comment_ranges.extend(get_leading_comment_ranges_of_node(
        &token_at_position,
        source_file,
    ));
    for comment_range in comment_ranges {
        if comment_range.text_range.contains_exclusive(position)
            || position == comment_range.end()
                && (comment_range.kind == ast::Kind::SingleLineCommentTrivia
                    || position as usize == source_file.text().len())
        {
            return Some(comment_range);
        }
    }
    None
}

pub fn get_block_indent(
    source_file: &ast::SourceFile,
    position: i32,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    // move backwards until we find a line with a non-whitespace character,
    // then find the first non-whitespace character for that line.
    let mut current = position;
    while current > 0 {
        let ch = source_file.text()[current as usize..]
            .chars()
            .next()
            .unwrap_or('\0');
        if !stringutil::is_white_space_like(ch) {
            break;
        }
        current -= ch.len_utf8() as i32;
    }

    let line_start = get_line_start_position_for_position(current, source_file);
    find_first_non_whitespace_column(line_start, current, source_file, options)
}

pub fn get_actual_indentation_for_list_item_before_comma(
    comma_token: &ast::Node,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    // previous token is comma that separates items in list - find the previous item and try to derive indentation from it
    if source_file.store().parent(*comma_token).is_none() {
        return -1;
    }
    let containing_list = get_containing_list(comma_token, source_file);
    let Some(containing_list) = containing_list else {
        return -1;
    };
    let comma_index = containing_list
        .iter()
        .position(|n| n == *comma_token)
        .map(|i| i as i32)
        .unwrap_or(-1);
    if comma_index > 0 {
        return derive_actual_indentation_from_list(
            &containing_list,
            comma_index - 1,
            source_file,
            options,
        );
    }
    -1
}

pub fn next_token_is_curly_brace_on_same_line_as_cursor(
    preceding_token: &ast::Node,
    current: &ast::Node,
    line_at_position: i32,
    source_file: &ast::SourceFile,
) -> NextTokenKind {
    let store = source_file.store();
    let next_token = astnav::find_next_token(*preceding_token, *current, source_file);
    let Some(next_token) = next_token else {
        return NEXT_TOKEN_KIND_UNKNOWN;
    };

    if store.kind(next_token) == ast::Kind::OpenBraceToken {
        // open braces are always indented at the parent level
        return NEXT_TOKEN_KIND_OPEN_BRACE;
    } else if store.kind(next_token) == ast::Kind::CloseBraceToken {
        // close braces are indented at the parent level if they are located on the same line with cursor
        let next_token_start_line = get_start_line_for_node(&next_token, source_file);
        if line_at_position == next_token_start_line {
            return NEXT_TOKEN_KIND_CLOSE_BRACE;
        }
        return NEXT_TOKEN_KIND_UNKNOWN;
    }

    NEXT_TOKEN_KIND_UNKNOWN
}

pub fn get_smart_indent(
    source_file: &ast::SourceFile,
    position: i32,
    preceding_token: &ast::Node,
    line_at_position: i32,
    assume_new_line_before_close_brace: bool,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    let store = source_file.store();
    // try to find node that can contribute to indentation and includes 'position' starting from 'precedingToken'
    // if such node is found - compute initial indentation for 'position' inside this node
    let mut previous = None;
    let mut current = Some(preceding_token.clone());

    while let Some(current_node) = current.clone() {
        if crate::lsutil::position_belongs_to_node(&current_node, position, source_file)
            && should_indent_child_node(
                options.clone(),
                &current_node,
                previous.as_ref(),
                source_file,
                true,
            )
        {
            let (current_start_line, current_start_char) =
                get_start_line_and_character_for_node(&current_node, source_file);
            let ntk = next_token_is_curly_brace_on_same_line_as_cursor(
                preceding_token,
                &current_node,
                line_at_position,
                source_file,
            );
            let mut indentation_delta = 0;
            if ntk != NEXT_TOKEN_KIND_UNKNOWN {
                // handle cases when codefix is about to be inserted before the close brace
                if assume_new_line_before_close_brace && ntk == NEXT_TOKEN_KIND_CLOSE_BRACE {
                    indentation_delta = options.indent_size;
                }
                // else 0
            } else if line_at_position != current_start_line {
                indentation_delta = options.indent_size;
            }
            return get_indentation_for_node_worker(
                &current_node,
                current_start_line,
                current_start_char,
                None,
                indentation_delta,
                source_file,
                true,
                options,
            );
        }

        // check if current node is a list item - if yes, take indentation from it
        // do not consider parent-child line sharing yet:
        // function foo(a
        //    | preceding node 'a' does share line with its parent but indentation is expected
        let actual_indentation =
            get_actual_indentation_for_list_item(&current_node, source_file, options.clone(), true);
        if actual_indentation != -1 {
            return actual_indentation;
        }

        previous = Some(current_node.clone());
        current = store.parent(current_node);
    }
    // no parent was found - return the base indentation of the SourceFile
    options.base_indent_size
}

pub fn get_indentation_for_node_worker(
    current: &ast::Node,
    mut current_start_line: i32,
    mut current_start_character: i32,
    ignore_actual_indentation_range: Option<&core::TextRange>,
    mut indentation_delta: i32,
    source_file: &ast::SourceFile,
    is_next_child: bool,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    let store = source_file.store();
    let mut current = current.clone();
    let mut parent = store.parent(current);

    // Walk up the tree and collect indentation for parent-child node pairs. Indentation is not added if
    // * parent and child nodes start on the same line, or
    // * parent is an IfStatement and child starts on the same line as an 'else clause'.
    while let Some(parent_node) = parent.clone() {
        let mut use_actual_indentation = true;
        if let Some(ignore_actual_indentation_range) = ignore_actual_indentation_range {
            let start = scanner::get_token_pos_of_node(&current, source_file, false) as i32;
            use_actual_indentation = start < ignore_actual_indentation_range.pos()
                || start > ignore_actual_indentation_range.end();
        }

        let (containing_list_or_parent_start_line, containing_list_or_parent_start_character) =
            get_containing_list_or_parent_start(&parent_node, &current, source_file);
        let parent_and_child_share_line = containing_list_or_parent_start_line
            == current_start_line
            || child_starts_on_the_same_line_with_else_in_if_statement(
                &parent_node,
                &current,
                current_start_line,
                source_file,
            );

        if use_actual_indentation {
            // check if current node is a list item - if yes, take indentation from it
            let mut first_list_child = None;
            let container_list = get_containing_list(&current, source_file);
            if let Some(container_list) = &container_list {
                first_list_child = container_list.first();
            }
            // A list indents its children if the children begin on a later line than the list itself:
            //
            // f1(               L0 - List start
            //   {               L1 - First child start: indented, along with all other children
            //     prop: 0
            //   },
            //   {
            //     prop: 1
            //   }
            // )
            //
            // f2({             L0 - List start and first child start: children are not indented.
            //   prop: 0             Object properties are indented only one level, because the list
            // }, {                  itself contributes nothing.
            //   prop: 1        L3 - The indentation of the second object literal is best understood by
            // })                    looking at the relationship between the list and *first* list item.
            let mut list_indents_child = false;
            if let Some(first_list_child) = &first_list_child {
                let list_line = get_start_line_for_node(first_list_child, source_file);
                list_indents_child = list_line > containing_list_or_parent_start_line;
            }
            let mut actual_indentation = get_actual_indentation_for_list_item(
                &current,
                source_file,
                options.clone(),
                list_indents_child,
            );
            if actual_indentation != -1 {
                return actual_indentation + indentation_delta;
            }

            // try to fetch actual indentation for current node from source text
            actual_indentation = get_actual_indentation_for_node(
                &current,
                &parent_node,
                current_start_line,
                current_start_character,
                parent_and_child_share_line,
                source_file,
                options.clone(),
            );
            if actual_indentation != -1 {
                return actual_indentation + indentation_delta;
            }
        }

        // increase indentation if parent node wants its content to be indented and parent and child nodes don't start on the same line
        if should_indent_child_node(
            options.clone(),
            &parent_node,
            Some(&current),
            source_file,
            is_next_child,
        ) && !parent_and_child_share_line
        {
            indentation_delta += options.indent_size;
        }

        // In our AST, a call argument's `parent` is the call-expression, not the argument list.
        // We would like to increase indentation based on the relationship between an argument and its argument-list,
        // so we spoof the starting position of the (parent) call-expression to match the (non-parent) argument-list.
        // But, the spoofed start-value could then cause a problem when comparing the start position of the call-expression
        // to *its* parent (in the case of an iife, an expression statement), adding an extra level of indentation.
        //
        // Instead, when at an argument, we unspoof the starting position of the enclosing call expression
        // *after* applying indentation for the argument.

        let use_true_start = is_argument_and_start_line_overlaps_expression_being_called(
            &parent_node,
            &current,
            current_start_line,
            source_file,
        );

        current = parent_node;
        parent = store.parent(current);

        if use_true_start {
            let (line, character) = scanner::get_ecma_line_and_byte_offset_of_position(
                source_file,
                scanner::get_token_pos_of_node(&current, source_file, false),
            );
            current_start_line = line as i32;
            current_start_character = character as i32;
        } else {
            current_start_line = containing_list_or_parent_start_line;
            current_start_character = containing_list_or_parent_start_character;
        }
    }

    indentation_delta + options.base_indent_size
}

/*
 * Function returns -1 if actual indentation for node should not be used (i.e because node is nested expression)
 */
pub fn get_actual_indentation_for_node(
    current: &ast::Node,
    parent: &ast::Node,
    current_line: i32,
    current_char: i32,
    parent_and_child_share_line: bool,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    // actual indentation is used for statements\declarations if one of cases below is true:
    // - parent is SourceFile - by default immediate children of SourceFile are not indented except when user indents them manually
    // - parent and child are not on the same line
    let store = source_file.store();
    let use_actual_indentation = (ast::is_declaration(store, *current)
        || ast::is_statement_but_not_declaration(store, *current))
        && (store.kind(*parent) == ast::Kind::SourceFile || !parent_and_child_share_line);

    if !use_actual_indentation {
        return -1;
    }

    find_column_for_first_non_whitespace_character_in_line(
        current_line,
        current_char,
        source_file,
        options,
    )
}

pub fn is_argument_and_start_line_overlaps_expression_being_called(
    parent: &ast::Node,
    child: &ast::Node,
    child_start_line: i32,
    source_file: &ast::SourceFile,
) -> bool {
    let store = source_file.store();
    if !(ast::is_call_expression(store, *parent)
        && store
            .arguments(*parent)
            .is_some_and(|arguments| arguments.iter().any(|argument| argument == *child)))
    {
        return false;
    }
    let expression_of_call_expression_end = store.loc(store.expression(*parent).unwrap()).end();
    let expression_of_call_expression_end_line =
        scanner::get_ecma_line_of_position(source_file, expression_of_call_expression_end) as i32;
    expression_of_call_expression_end_line == child_start_line
}

pub fn get_actual_indentation_for_list_item(
    node: &ast::Node,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
    list_indents_child: bool,
) -> i32 {
    if source_file.store().parent(*node).is_some_and(|parent| {
        source_file.store().kind(parent) == ast::Kind::VariableDeclarationList
    }) {
        // VariableDeclarationList has no wrapping tokens
        return -1;
    }
    let containing_list = get_containing_list(node, source_file);
    if let Some(containing_list) = containing_list {
        if let Some(index) = containing_list.iter().position(|e| e == *node) {
            let result = derive_actual_indentation_from_list(
                &containing_list,
                index as i32,
                source_file,
                options.clone(),
            );
            if result != -1 {
                return result;
            }
        }
        let mut delta = 0;
        if list_indents_child {
            delta = options.indent_size;
        }
        let res =
            get_actual_indentation_for_list_start_line(&containing_list, source_file, options);
        if res == -1 {
            return delta;
        }
        return res + delta;
    }
    -1
}

pub fn get_actual_indentation_for_list_start_line(
    list: &ast::SourceNodeList<'_>,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    let (line, char_) =
        scanner::get_ecma_line_and_byte_offset_of_position(source_file, list.loc().pos());
    find_column_for_first_non_whitespace_character_in_line(
        line as i32,
        char_ as i32,
        source_file,
        options,
    )
}

pub fn derive_actual_indentation_from_list(
    list: &ast::SourceNodeList<'_>,
    index: i32,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    assert!(index >= 0 && (index as usize) < list.len());

    let nodes: Vec<_> = list.iter().collect();
    let node = &nodes[index as usize];

    // walk toward the start of the list starting from current node and check if the line is the same for all items.
    // if end line for item [i - 1] differs from the start line for item [i] - find column of the first non-whitespace character on the line of item [i]

    let (mut line, mut char_) = get_start_line_and_character_for_node(node, source_file);

    for i in (0..=index as usize).rev() {
        if source_file.store().kind(nodes[i]) == ast::Kind::CommaToken {
            continue;
        }
        // skip list items that ends on the same line with the current list element
        let prev_end_line = scanner::get_ecma_line_of_position(
            source_file,
            source_file.store().loc(nodes[i]).end(),
        ) as i32;
        if prev_end_line != line {
            return find_column_for_first_non_whitespace_character_in_line(
                line,
                char_,
                source_file,
                options,
            );
        }

        (line, char_) = get_start_line_and_character_for_node(&nodes[i], source_file);
    }
    -1
}

pub fn find_column_for_first_non_whitespace_character_in_line(
    line: i32,
    char_: i32,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    let line_start =
        scanner::get_ecma_position_of_line_and_byte_offset(source_file, line as usize, 0) as i32;
    find_first_non_whitespace_column(line_start, line_start + char_, source_file, options)
}

pub fn find_first_non_whitespace_column(
    start_pos: i32,
    end_pos: i32,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> i32 {
    let (_, col) =
        find_first_non_whitespace_character_and_column(start_pos, end_pos, source_file, options);
    col
}

/**
 * Character is the actual index of the character since the beginning of the line.
 * Column - position of the character after expanding tabs to spaces.
 * "0\t2$"
 * value of 'character' for '$' is 3
 * value of 'column' for '$' is 6 (assuming that tab size is 4)
 */
pub fn find_first_non_whitespace_character_and_column(
    start_pos: i32,
    end_pos: i32,
    source_file: &ast::SourceFile,
    options: crate::lsutil::FormatCodeSettings,
) -> (i32, i32) {
    let mut column = 0;
    let text = source_file.text();
    let mut pos = start_pos;
    while pos < end_pos {
        let ch = text[pos as usize..].chars().next().unwrap_or('\0');
        if !stringutil::is_white_space_single_line(ch) {
            break;
        }

        if ch == '\t' {
            if options.tab_size > 0 {
                column += options.tab_size + (column % options.tab_size);
            }
        } else {
            column += 1;
        }

        pos += ch.len_utf8() as i32;
    }
    (pos - start_pos, column)
}

pub fn child_starts_on_the_same_line_with_else_in_if_statement(
    parent: &ast::Node,
    child: &ast::Node,
    child_start_line: i32,
    source_file: &ast::SourceFile,
) -> bool {
    let store = source_file.store();
    if store.kind(*parent) == ast::Kind::IfStatement
        && store.else_statement(*parent).as_ref() == Some(child)
    {
        let else_keyword = astnav::find_preceding_token(source_file, store.loc(*child).pos());
        assert!(else_keyword.is_some());
        let else_keyword_start_line = get_start_line_for_node(&else_keyword.unwrap(), source_file);
        return else_keyword_start_line == child_start_line;
    }
    false
}

pub fn get_start_line_and_character_for_node(
    n: &ast::Node,
    source_file: &ast::SourceFile,
) -> (i32, i32) {
    let (line, character) = scanner::get_ecma_line_and_byte_offset_of_position(
        source_file,
        scanner::get_token_pos_of_node(n, source_file, false),
    );
    (line as i32, character as i32)
}

pub fn get_start_line_for_node(n: &ast::Node, source_file: &ast::SourceFile) -> i32 {
    scanner::get_ecma_line_of_position(
        source_file,
        scanner::get_token_pos_of_node(n, source_file, false),
    ) as i32
}

pub fn get_containing_list<'a>(
    node: &ast::Node,
    source_file: &'a ast::SourceFile,
) -> Option<ast::SourceNodeList<'a>> {
    let start = scanner::get_token_pos_of_node(node, source_file, false) as i32;
    let end = source_file.store().loc(*node).end();
    let Some(parent) = source_file.store().parent(*node) else {
        return None;
    };
    get_list_by_range(start, end, &parent, source_file)
}

pub fn get_list_by_position<'a>(
    pos: i32,
    node: Option<&ast::Node>,
    source_file: &'a ast::SourceFile,
) -> Option<ast::SourceNodeList<'a>> {
    let node = node?;
    get_list_by_range(pos, pos, node, source_file)
}

pub fn get_list_by_range<'a>(
    start: i32,
    end: i32,
    node: &ast::Node,
    source_file: &'a ast::SourceFile,
) -> Option<ast::SourceNodeList<'a>> {
    let r = core::new_text_range(start, end);
    let store = source_file.store();
    match store.kind(*node) {
        ast::Kind::TypeReference => get_list(store.type_arguments(*node), r, node, source_file),
        ast::Kind::ObjectLiteralExpression => {
            get_list(store.properties(*node), r, node, source_file)
        }
        ast::Kind::ArrayLiteralExpression => get_list(store.elements(*node), r, node, source_file),
        ast::Kind::TypeLiteral => get_list(store.members(*node), r, node, source_file),
        ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::CallSignature
        | ast::Kind::Constructor
        | ast::Kind::ConstructorType
        | ast::Kind::ConstructSignature => {
            let tpl = get_list(store.type_parameters(*node), r, node, source_file);
            if tpl.is_some() {
                return tpl;
            }
            get_list(store.parameters(*node), r, node, source_file)
        }
        ast::Kind::GetAccessor => get_list(store.parameters(*node), r, node, source_file),
        ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::TypeAliasDeclaration => {
            get_list(store.type_parameters(*node), r, node, source_file)
        }
        ast::Kind::NewExpression | ast::Kind::CallExpression => {
            let l = get_list(store.type_arguments(*node), r, node, source_file);
            if l.is_some() {
                return l;
            }
            get_list(store.arguments(*node), r, node, source_file)
        }
        ast::Kind::VariableDeclarationList => {
            get_list(store.declarations(*node), r, node, source_file)
        }
        ast::Kind::ObjectBindingPattern
        | ast::Kind::ArrayBindingPattern
        | ast::Kind::NamedImports
        | ast::Kind::NamedExports => get_list(store.elements(*node), r, node, source_file),
        _ => None,
    }
}

pub fn get_list<'a>(
    list: Option<ast::SourceNodeList<'a>>,
    r: core::TextRange,
    node: &ast::Node,
    source_file: &ast::SourceFile,
) -> Option<ast::SourceNodeList<'a>> {
    let list = list?;
    let visual = get_visual_list_range(node, list.loc(), source_file);
    if r.contained_by(visual) {
        return Some(list);
    }
    None
}

pub fn get_visual_list_range(
    _node: &ast::Node,
    list: core::TextRange,
    source_file: &ast::SourceFile,
) -> core::TextRange {
    // In strada, this relied on the services .getChildren method, which manifested synthetic token nodes
    // _however_, the logic boils down to "find the child with the matching span and adjust its start to the
    // previous (possibly token) child's end and its end to the token start of the following element" - basically
    // expanding the range to encompass all the neighboring non-token trivia
    // Now, we perform that logic with the scanner instead
    let prior = astnav::find_preceding_token(source_file, list.pos());
    let prior_end = prior
        .map_or(list.pos(), |prior| source_file.store().loc(prior).end())
        .min(list.pos());
    // Find the token that starts at or after list.End() using the scanner
    let scan = scanner::get_scanner_for_source_file(source_file, list.end() as usize);
    let next_start = if scan.token() == ast::Kind::EndOfFile {
        list.end()
    } else {
        scan.token_start()
    }
    .max(list.end());
    core::new_text_range(prior_end, next_start)
}

pub fn get_containing_list_or_parent_start(
    parent: &ast::Node,
    child: &ast::Node,
    source_file: &ast::SourceFile,
) -> (i32, i32) {
    let containing_list = get_containing_list(child, source_file);
    let start_pos = if let Some(containing_list) = containing_list {
        containing_list.loc().pos()
    } else {
        scanner::get_token_pos_of_node(parent, source_file, false) as i32
    };
    let (line, character) =
        scanner::get_ecma_line_and_byte_offset_of_position(source_file, start_pos);
    (line as i32, character as i32)
}

pub fn is_control_flow_ending_statement(kind: ast::Kind, parent_kind: ast::Kind) -> bool {
    match kind {
        ast::Kind::ReturnStatement
        | ast::Kind::ThrowStatement
        | ast::Kind::ContinueStatement
        | ast::Kind::BreakStatement => parent_kind != ast::Kind::Block,
        _ => false,
    }
}

/**
 * True when the parent node should indent the given child by an explicit rule.
 * @param isNextChild If true, we are judging indent of a hypothetical child *after* this one, not the current child.
 */
pub fn should_indent_child_node(
    settings: crate::lsutil::FormatCodeSettings,
    parent: &ast::Node,
    child: Option<&ast::Node>,
    source_file: &ast::SourceFile,
    is_next_child: bool,
) -> bool {
    node_will_indent_child(settings, parent, child, Some(source_file), false)
        && !(is_next_child
            && child.is_some_and(|child| {
                is_control_flow_ending_statement(
                    source_file.store().kind(*child),
                    source_file.store().kind(*parent),
                )
            }))
}

pub fn node_will_indent_child(
    settings: crate::lsutil::FormatCodeSettings,
    parent: &ast::Node,
    child: Option<&ast::Node>,
    source_file: Option<&ast::SourceFile>,
    indent_by_default: bool,
) -> bool {
    let child_kind = child.map_or(ast::Kind::Unknown, |child| {
        source_file.unwrap().store().kind(*child)
    });

    match source_file.unwrap().store().kind(*parent) {
        ast::Kind::ExpressionStatement
        | ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::ArrayLiteralExpression
        | ast::Kind::Block
        | ast::Kind::ModuleBlock
        | ast::Kind::ObjectLiteralExpression
        | ast::Kind::TypeLiteral
        | ast::Kind::MappedType
        | ast::Kind::TupleType
        | ast::Kind::ParenthesizedExpression
        | ast::Kind::PropertyAccessExpression
        | ast::Kind::CallExpression
        | ast::Kind::NewExpression
        | ast::Kind::VariableStatement
        | ast::Kind::ExportAssignment
        | ast::Kind::ReturnStatement
        | ast::Kind::ConditionalExpression
        | ast::Kind::ArrayBindingPattern
        | ast::Kind::ObjectBindingPattern
        | ast::Kind::JsxOpeningElement
        | ast::Kind::JsxOpeningFragment
        | ast::Kind::JsxSelfClosingElement
        | ast::Kind::JsxExpression
        | ast::Kind::MethodSignature
        | ast::Kind::CallSignature
        | ast::Kind::ConstructSignature
        | ast::Kind::Parameter
        | ast::Kind::FunctionType
        | ast::Kind::ConstructorType
        | ast::Kind::ParenthesizedType
        | ast::Kind::TaggedTemplateExpression
        | ast::Kind::AwaitExpression
        | ast::Kind::NamedExports
        | ast::Kind::NamedImports
        | ast::Kind::ExportSpecifier
        | ast::Kind::ImportSpecifier
        | ast::Kind::PropertyDeclaration
        | ast::Kind::CaseClause
        | ast::Kind::DefaultClause => true,
        ast::Kind::CaseBlock => settings.indent_switch_case.is_true_or_unknown(),
        ast::Kind::VariableDeclaration
        | ast::Kind::PropertyAssignment
        | ast::Kind::BinaryExpression => {
            if settings
                .indent_multi_line_object_literal_beginning_on_blank_line
                .is_false_or_unknown()
                && source_file.is_some()
                && child_kind == ast::Kind::ObjectLiteralExpression
            {
                let source_file = source_file.unwrap();
                return range_is_on_one_line(source_file.store().loc(*child.unwrap()), source_file);
            }
            if source_file.unwrap().store().kind(*parent) == ast::Kind::BinaryExpression
                && source_file.is_some()
                && child_kind == ast::Kind::JsxElement
            {
                let source_file = source_file.unwrap();
                let parent_start_line = scanner::get_ecma_line_of_position(
                    source_file,
                    scanner::skip_trivia(
                        source_file.text(),
                        source_file.store().loc(*parent).pos() as usize,
                    ),
                );
                let child_start_line = scanner::get_ecma_line_of_position(
                    source_file,
                    scanner::skip_trivia(
                        source_file.text(),
                        source_file.store().loc(*child.unwrap()).pos() as usize,
                    ),
                );
                return parent_start_line != child_start_line;
            }
            if source_file.unwrap().store().kind(*parent) != ast::Kind::BinaryExpression {
                return true;
            }
            indent_by_default
        }
        ast::Kind::DoStatement
        | ast::Kind::WhileStatement
        | ast::Kind::ForInStatement
        | ast::Kind::ForOfStatement
        | ast::Kind::ForStatement
        | ast::Kind::IfStatement
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::MethodDeclaration
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor => child_kind != ast::Kind::Block,
        ast::Kind::ArrowFunction => {
            if source_file.is_some() && child_kind == ast::Kind::ParenthesizedExpression {
                let source_file = source_file.unwrap();
                return range_is_on_one_line(source_file.store().loc(*child.unwrap()), source_file);
            }
            child_kind != ast::Kind::Block
        }
        ast::Kind::ExportDeclaration => child_kind != ast::Kind::NamedExports,
        ast::Kind::ImportDeclaration => {
            child_kind != ast::Kind::ImportClause
                || source_file.is_some_and(|source_file| {
                    child
                        .and_then(|child| source_file.store().named_bindings(*child))
                        .as_ref()
                        .is_some_and(|bindings| {
                            source_file.store().kind(*bindings) != ast::Kind::NamedImports
                        })
                })
        }
        ast::Kind::JsxElement => child_kind != ast::Kind::JsxClosingElement,
        ast::Kind::JsxFragment => child_kind != ast::Kind::JsxClosingFragment,
        ast::Kind::IntersectionType | ast::Kind::UnionType | ast::Kind::SatisfiesExpression => {
            if child_kind == ast::Kind::TypeLiteral
                || child_kind == ast::Kind::TupleType
                || child_kind == ast::Kind::MappedType
            {
                return false;
            }
            indent_by_default
        }
        ast::Kind::TryStatement => {
            if child_kind == ast::Kind::Block {
                return false;
            }
            indent_by_default
        }
        _ => indent_by_default,
    }
}

// A multiline conditional typically increases the indentation of its whenTrue and whenFalse children:
//
// condition
//
//	? whenTrue
//	: whenFalse;
//
// However, that indentation does not apply if the subexpressions themselves span multiple lines,
// applying their own indentation:
//
//	(() => {
//	  return complexCalculationForCondition();
//	})() ? {
//
//	  whenTrue: 'multiline object literal'
//	} : (
//
//	whenFalse('multiline parenthesized expression')
//
// );
//
// In these cases, we must discard the indentation increase that would otherwise be applied to the
// whenTrue and whenFalse children to avoid double-indenting their contents. To identify this scenario,
// we check for the whenTrue branch beginning on the line that the condition ends, and the whenFalse
// branch beginning on the line that the whenTrue branch ends.
pub fn child_is_unindented_branch_of_conditional_expression(
    parent: &ast::Node,
    child: &ast::Node,
    child_start_line: i32,
    source_file: &ast::SourceFile,
) -> bool {
    let store = source_file.store();
    if store.kind(*parent) == ast::Kind::ConditionalExpression
        && (Some(*child) == store.when_true(*parent) || Some(*child) == store.when_false(*parent))
    {
        let condition_end_line = scanner::get_ecma_line_of_position(
            source_file,
            store
                .condition(*parent)
                .map_or(store.loc(*parent).pos(), |condition| {
                    store.loc(condition).end()
                }),
        );
        let condition_end_line = condition_end_line as i32;
        if Some(*child) == store.when_true(*parent) {
            return child_start_line == condition_end_line;
        } else {
            // On the whenFalse side, we have to look at the whenTrue side, because if that one was
            // indented, whenFalse must also be indented:
            //
            // const y = true
            //   ? 1 : (          L1: whenTrue indented because it's on a new line
            //     0              L2: indented two stops, one because whenTrue was indented
            //   );                   and one because of the parentheses spanning multiple lines
            let true_start_line =
                get_start_line_for_node(&store.when_true(*parent).unwrap(), source_file);
            let true_end_line = scanner::get_ecma_line_of_position(
                source_file,
                store.loc(store.when_true(*parent).unwrap()).end(),
            ) as i32;
            return condition_end_line == true_start_line && true_end_line == child_start_line;
        }
    }
    false
}

pub fn argument_starts_on_same_line_as_previous_argument(
    parent: &ast::Node,
    child: &ast::Node,
    child_start_line: i32,
    source_file: &ast::SourceFile,
) -> bool {
    let store = source_file.store();
    if ast::is_call_expression(store, *parent) || ast::is_new_expression(store, *parent) {
        let Some(arguments) = store.arguments(*parent) else {
            return false;
        };
        if arguments.is_empty() {
            return false;
        }
        let current_index = arguments
            .iter()
            .position(|n| n == *child)
            .map(|i| i as i32)
            .unwrap_or(-1);
        if current_index == -1 {
            // If it's not one of the arguments, don't look past this
            return false;
        }
        if current_index == 0 {
            return false; // Can't look at previous node if first
        }

        let previous_node = arguments.iter().nth(current_index as usize - 1).unwrap();
        let line_of_previous_node =
            scanner::get_ecma_line_of_position(source_file, store.loc(previous_node).end()) as i32;
        if child_start_line == line_of_previous_node {
            return true;
        }
    }
    false
}
