use ts_ast as ast;
use ts_core as core;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::{indent, scanner as formatting_scanner, span as format_span_impl, util};

pub type Context = std::collections::HashMap<FormatContextKey, ContextValue>;
pub type FormatContext = Context;

#[derive(Clone)]
pub enum ContextValue {
    FormatCodeSettings(crate::lsutil::FormatCodeSettings),
    String(String),
}

pub type FormatRequestKind = i32;

pub const FORMAT_REQUEST_KIND_FORMAT_DOCUMENT: FormatRequestKind = 0;
pub const FORMAT_REQUEST_KIND_FORMAT_SELECTION: FormatRequestKind = 1;
pub const FORMAT_REQUEST_KIND_FORMAT_ON_ENTER: FormatRequestKind = 2;
pub const FORMAT_REQUEST_KIND_FORMAT_ON_SEMICOLON: FormatRequestKind = 3;
pub const FORMAT_REQUEST_KIND_FORMAT_ON_OPENING_CURLY_BRACE: FormatRequestKind = 4;
pub const FORMAT_REQUEST_KIND_FORMAT_ON_CLOSING_CURLY_BRACE: FormatRequestKind = 5;

pub type FormatContextKey = i32;

pub const FORMAT_OPTIONS_KEY: FormatContextKey = 0;
pub const FORMAT_NEWLINE_KEY: FormatContextKey = 1;

pub fn with_format_code_settings(
    mut ctx: Context,
    options: crate::lsutil::FormatCodeSettings,
    new_line: String,
) -> Context {
    ctx.insert(
        FORMAT_OPTIONS_KEY,
        ContextValue::FormatCodeSettings(options),
    );
    ctx.insert(FORMAT_NEWLINE_KEY, ContextValue::String(new_line));
    // In strada, the rules map was both globally cached *and* cached into the context, for some reason. We skip that here and just use the global one.
    ctx
}

pub fn get_format_code_settings_from_context(ctx: &Context) -> crate::lsutil::FormatCodeSettings {
    if let Some(ContextValue::FormatCodeSettings(opt)) = ctx.get(&FORMAT_OPTIONS_KEY) {
        return opt.clone();
    }
    crate::lsutil::get_default_format_code_settings()
}

pub fn get_new_line_or_default_from_context(ctx: &Context) -> String {
    let opt = get_format_code_settings_from_context(ctx);
    if !opt.new_line_character.is_empty() {
        return opt.new_line_character;
    }
    let Some(ContextValue::String(host)) = ctx.get(&FORMAT_NEWLINE_KEY) else {
        panic!("format newline missing from context");
    };
    if !host.is_empty() {
        return host.clone();
    }
    "\n".to_owned()
}

fn decode_rune_in_string_at(text: &str, index: i32) -> (char, usize) {
    let index = index as usize;
    if !text.is_char_boundary(index) {
        return (char::REPLACEMENT_CHARACTER, 1);
    }
    let mut chars = text[index..].chars();
    if let Some(ch) = chars.next() {
        return (ch, ch.len_utf8());
    }
    (char::REPLACEMENT_CHARACTER, 0)
}

pub fn format_span(
    ctx: &Context,
    span: core::TextRange,
    file: &ast::SourceFile,
    kind: FormatRequestKind,
) -> Vec<core::TextChange> {
    // find the smallest node that fully wraps the range and compute the initial indentation for the node
    let enclosing_node = format_span_impl::find_enclosing_node(span, file);
    let opts = get_format_code_settings_from_context(ctx);
    let mut worker = format_span_impl::new_format_span_worker(
        ctx.clone(),
        span,
        enclosing_node.clone(),
        indent::get_indentation_for_node(&enclosing_node, Some(&span), file, opts.clone()),
        format_span_impl::get_own_or_inherited_delta(Some(enclosing_node.clone()), opts, file),
        kind,
        format_span_impl::prepare_range_contains_error_function(file.diagnostics().to_vec(), span),
        file.share_readonly(),
    );

    formatting_scanner::new_formatting_scanner(
        file.text().to_owned(),
        file.language_variant(),
        format_span_impl::get_scan_start_position(&enclosing_node, span, file),
        span.end(),
        &mut worker,
    )
}

pub fn format_node_given_indentation(
    ctx: &Context,
    node: &ast::Node,
    file: &ast::SourceFile,
    language_variant: core::LanguageVariant,
    initial_indentation: i32,
    delta: i32,
) -> Vec<core::TextChange> {
    let text_range = file.store().loc(*node);
    let mut worker = format_span_impl::new_format_span_worker(
        ctx.clone(),
        text_range,
        node.clone(),
        initial_indentation,
        delta,
        FORMAT_REQUEST_KIND_FORMAT_SELECTION,
        Box::new(|_range| false), // assume that node does not have any errors
        file.share_readonly(),
    );
    formatting_scanner::new_formatting_scanner(
        file.text().to_owned(),
        language_variant,
        text_range.pos(),
        text_range.end(),
        &mut worker,
    )
}

pub fn format_node_lines(
    ctx: &Context,
    source_file: &ast::SourceFile,
    node: Option<&ast::Node>,
    request_kind: FormatRequestKind,
) -> Vec<core::TextChange> {
    let Some(node) = node else {
        return Vec::new();
    };
    let token_start = scanner::get_token_pos_of_node(node, source_file, false);
    let line_start = util::get_line_start_position_for_position(token_start as i32, source_file);
    let span = core::new_text_range(line_start, source_file.store().loc(*node).end());
    format_span(ctx, span, source_file, request_kind)
}

pub fn format_document(ctx: &Context, source_file: &ast::SourceFile) -> Vec<core::TextChange> {
    format_span(
        ctx,
        core::new_text_range(0, source_file.store().loc(source_file.as_node()).end()),
        source_file,
        FORMAT_REQUEST_KIND_FORMAT_DOCUMENT,
    )
}

pub fn format_selection(
    ctx: &Context,
    source_file: &ast::SourceFile,
    start: i32,
    end: i32,
) -> Vec<core::TextChange> {
    format_span(
        ctx,
        core::new_text_range(
            util::get_line_start_position_for_position(start, source_file),
            end,
        ),
        source_file,
        FORMAT_REQUEST_KIND_FORMAT_SELECTION,
    )
}

pub fn format_on_opening_curly(
    ctx: &Context,
    source_file: &ast::SourceFile,
    position: i32,
) -> Vec<core::TextChange> {
    let opening_curly = util::find_immediately_preceding_token_of_kind(
        position,
        ast::Kind::OpenBraceToken,
        source_file,
    );
    let Some(opening_curly) = opening_curly else {
        return Vec::new();
    };
    let Some(curly_brace_range) = opening_curly
        .node
        .and_then(|node| source_file.store().parent(node))
        .or(opening_curly.parent)
    else {
        return Vec::new();
    };
    let outermost_node =
        util::find_outermost_node_within_list_level(source_file.store(), &curly_brace_range);
    /*
     * We limit the span to end at the opening curly to handle the case where
     * the brace matched to that just typed will be incorrect after further edits.
     * For example, we could type the opening curly for the following method
     * body without brace-matching activated:
     * ```
     * class C {
     *     foo()
     * }
     * ```
     * and we wouldn't want to move the closing brace.
     */
    let text_range = core::new_text_range(
        util::get_line_start_position_for_position(
            scanner::get_token_pos_of_node(&outermost_node, source_file, false) as i32,
            source_file,
        ),
        position,
    );
    format_span(
        ctx,
        text_range,
        source_file,
        FORMAT_REQUEST_KIND_FORMAT_ON_OPENING_CURLY_BRACE,
    )
}

pub fn format_on_closing_curly(
    ctx: &Context,
    source_file: &ast::SourceFile,
    position: i32,
) -> Vec<core::TextChange> {
    let preceding_token = util::find_immediately_preceding_token_of_kind(
        position,
        ast::Kind::CloseBraceToken,
        source_file,
    );
    let outermost_node = preceding_token
        .as_ref()
        .and_then(|token| token.node.or(token.parent))
        .map(|node| util::find_outermost_node_within_list_level(source_file.store(), &node));
    format_node_lines(
        ctx,
        source_file,
        outermost_node.as_ref(),
        FORMAT_REQUEST_KIND_FORMAT_ON_CLOSING_CURLY_BRACE,
    )
}

pub fn format_on_semicolon(
    ctx: &Context,
    source_file: &ast::SourceFile,
    position: i32,
) -> Vec<core::TextChange> {
    let semicolon = util::find_immediately_preceding_token_of_kind(
        position,
        ast::Kind::SemicolonToken,
        source_file,
    );
    let outermost_node = semicolon
        .as_ref()
        .and_then(|token| token.node.or(token.parent))
        .map(|node| util::find_outermost_node_within_list_level(source_file.store(), &node));
    format_node_lines(
        ctx,
        source_file,
        outermost_node.as_ref(),
        FORMAT_REQUEST_KIND_FORMAT_ON_SEMICOLON,
    )
}

pub fn format_on_enter(
    ctx: &Context,
    source_file: &ast::SourceFile,
    position: i32,
) -> Vec<core::TextChange> {
    let line = scanner::get_ecma_line_of_position(source_file, position as usize);
    if line == 0 {
        return Vec::new();
    }
    // get start position for the previous line
    let start_pos = scanner::get_ecma_line_starts(source_file)[line as usize - 1] as i32;
    // After the enter key, the cursor is now at a new line. The new line may or may not contain non-whitespace characters.
    // If the new line has only whitespaces, we won't want to format this line, because that would remove the indentation as
    // trailing whitespaces. So the end of the formatting span should be the later one between:
    //  1. the end of the previous line
    //  2. the last non-whitespace character in the current line
    let mut end_of_format_span = scanner::get_ecma_end_line_position(source_file, line) as i32;
    while end_of_format_span > start_pos {
        let (ch, size) = decode_rune_in_string_at(source_file.text(), end_of_format_span);
        if size == 0 || stringutil::is_white_space_single_line(ch) {
            // on multibyte character keep backing up
            end_of_format_span -= 1;
            continue;
        }
        break;
    }

    // if the character at the end of the span is a line break, we shouldn't include it, because it indicates we don't want to
    // touch the current line at all. Also, on some OSes the line break consists of two characters (\r\n), we should test if the
    // previous character before the end of format span is line break character as well.
    let (ch, _) = decode_rune_in_string_at(source_file.text(), end_of_format_span);
    if stringutil::is_line_break(ch) {
        end_of_format_span -= 1;
    }

    let span = core::new_text_range(
        start_pos,
        // end value is exclusive so add 1 to the result
        end_of_format_span + 1,
    );

    format_span(ctx, span, source_file, FORMAT_REQUEST_KIND_FORMAT_ON_ENTER)
}
