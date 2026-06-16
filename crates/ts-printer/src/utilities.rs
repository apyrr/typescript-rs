use std::sync::Arc;

use ts_ast as ast;
use ts_core as core;
use ts_scanner as scanner;
use ts_sourcemap as sourcemap;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GetLiteralTextFlags(pub u32);

impl GetLiteralTextFlags {
    pub const NONE: Self = Self(0);
    pub const NEVER_ASCII_ESCAPE: Self = Self(1 << 0);
    pub const JSX_ATTRIBUTE_ESCAPE: Self = Self(1 << 1);
    pub const TERMINATE_UNTERMINATED_LITERALS: Self = Self(1 << 2);
    pub const ALLOW_NUMERIC_SEPARATOR: Self = Self(1 << 3);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for GetLiteralTextFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for GetLiteralTextFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteChar {
    SingleQuote,
    DoubleQuote,
    Backtick,
}

impl QuoteChar {
    fn as_char(self) -> char {
        match self {
            Self::SingleQuote => '\'',
            Self::DoubleQuote => '"',
            Self::Backtick => '`',
        }
    }
}

fn jsx_escaped_char(ch: char) -> Option<&'static str> {
    match ch {
        '"' => Some("&quot;"),
        '\'' => Some("&apos;"),
        _ => None,
    }
}

fn escaped_char(ch: char) -> Option<&'static str> {
    match ch {
        '\t' => Some(r"\t"),
        '\u{000B}' => Some(r"\v"),
        '\u{000C}' => Some(r"\f"),
        '\u{0008}' => Some(r"\b"),
        '\r' => Some(r"\r"),
        '\n' => Some(r"\n"),
        '\\' => Some(r"\\"),
        '"' => Some("\\\""),
        '\'' => Some("\\'"),
        '`' => Some("\\`"),
        '$' => Some(r"\$"),
        '\u{2028}' => Some(r"\u2028"),
        '\u{2029}' => Some(r"\u2029"),
        '\u{0085}' => Some(r"\u0085"),
        _ => None,
    }
}

fn encode_jsx_character_entity(b: &mut String, char_code: char) {
    b.push_str("&#x");
    b.push_str(&format!("{:X}", char_code as u32));
    b.push(';');
}

fn encode_utf16_escape_sequence(b: &mut String, char_code: u32) {
    b.push_str(r"\u");
    let hex = format!("{char_code:X}");
    for _ in hex.len()..4 {
        b.push('0');
    }
    b.push_str(&hex);
}

// Based heavily on the abstract 'Quote'/'QuoteJSONString' operation from ECMA-262 (24.3.2.2),
// but augmented for a few select characters (e.g. lineSeparator, paragraphSeparator, nextLine)
// Note that this doesn't actually wrap the input in double quotes.
fn escape_string_worker(
    s: &str,
    quote_char: QuoteChar,
    flags: GetLiteralTextFlags,
    b: &mut String,
) {
    let mut pos = 0;
    let mut i = 0;
    let quote_char_value = quote_char.as_char();

    while i < s.len() {
        let ch = s[i..].chars().next().unwrap();
        let size = ch.len_utf8();
        let mut escape = false;

        match ch {
            '\\' => {
                if !flags.contains(GetLiteralTextFlags::JSX_ATTRIBUTE_ESCAPE) {
                    escape = true;
                }
            }
            '$' => {
                if quote_char == QuoteChar::Backtick
                    && i + 1 < s.len()
                    && s.as_bytes()[i + 1] == b'{'
                {
                    escape = true;
                }
            }
            '\u{2028}' | '\u{2029}' | '\u{0085}' | '\r' => escape = true,
            '\n' => {
                if quote_char != QuoteChar::Backtick {
                    escape = true;
                }
            }
            _ if ch == quote_char_value => escape = true,
            _ => {
                if ch <= '\u{001F}'
                    || (!flags.contains(GetLiteralTextFlags::NEVER_ASCII_ESCAPE) && ch > '\u{007F}')
                {
                    escape = true;
                }
            }
        }

        if escape {
            if pos < i {
                b.push_str(&s[pos..i]);
            }

            if flags.contains(GetLiteralTextFlags::JSX_ATTRIBUTE_ESCAPE) {
                if ch == '\0' {
                    b.push_str("&#0;");
                } else if let Some(matched) = jsx_escaped_char(ch) {
                    b.push_str(matched);
                } else {
                    encode_jsx_character_entity(b, ch);
                }
            } else if ch == '\r'
                && quote_char == QuoteChar::Backtick
                && i + 1 < s.len()
                && s.as_bytes()[i + 1] == b'\n'
            {
                b.push_str(r"\r\n");
                pos = i + size + 1;
                i += size + 1;
                continue;
            } else if (ch as u32) > 0xFFFF {
                let codepoint = ch as u32 - 0x10000;
                encode_utf16_escape_sequence(b, ((codepoint >> 10) & 0x3FF) + 0xD800);
                encode_utf16_escape_sequence(b, (codepoint & 0x3FF) + 0xDC00);
            } else if ch == '\0' {
                let next_is_digit = if i + size < s.len() {
                    s[i + size..]
                        .chars()
                        .next()
                        .is_some_and(stringutil::is_digit)
                } else {
                    false
                };
                if next_is_digit {
                    b.push_str(r"\x00");
                } else {
                    b.push_str(r"\0");
                }
            } else if let Some(matched) = escaped_char(ch) {
                b.push_str(matched);
            } else {
                encode_utf16_escape_sequence(b, ch as u32);
            }

            pos = i + size;
        }

        i += size;
    }

    if pos < i {
        b.push_str(&s[pos..]);
    }
}

pub fn escape_string(s: String, quote_char: QuoteChar) -> String {
    let mut b = String::with_capacity(s.len() + 2);
    escape_string_worker(
        &s,
        quote_char,
        GetLiteralTextFlags::NEVER_ASCII_ESCAPE,
        &mut b,
    );
    b
}

pub(crate) fn escape_non_ascii_string(s: String, quote_char: QuoteChar) -> String {
    let mut b = String::with_capacity(s.len() + 2);
    escape_string_worker(&s, quote_char, GetLiteralTextFlags::NONE, &mut b);
    b
}

pub(crate) fn escape_jsx_attribute_string(s: String, quote_char: QuoteChar) -> String {
    let mut b = String::with_capacity(s.len() + 2);
    escape_string_worker(
        &s,
        quote_char,
        GetLiteralTextFlags::JSX_ATTRIBUTE_ESCAPE | GetLiteralTextFlags::NEVER_ASCII_ESCAPE,
        &mut b,
    );
    b
}

pub(crate) fn can_use_original_text(
    store: &ast::AstStore,
    node: &ast::Node,
    flags: GetLiteralTextFlags,
) -> bool {
    if ast::node_is_synthesized(store, *node)
        || store.flags(*node).intersects(ast::NODE_FLAGS_SYNTHESIZED)
        || store.parent(*node).is_none()
        || (flags.contains(GetLiteralTextFlags::TERMINATE_UNTERMINATED_LITERALS)
            && ast::is_unterminated_literal(store, *node))
    {
        return false;
    }

    if store.kind(*node) == ast::Kind::NumericLiteral {
        let token_flags = store
            .token_flags(*node)
            .expect("numeric literal should have token flags");
        if token_flags.intersects(ast::TokenFlags::IS_INVALID) {
            return false;
        }
        if token_flags.contains(ast::TokenFlags::CONTAINS_SEPARATOR) {
            return flags.contains(GetLiteralTextFlags::ALLOW_NUMERIC_SEPARATOR);
        }
    }

    store.kind(*node) != ast::Kind::BigIntLiteral
}

pub(crate) fn get_literal_text(
    store: &ast::AstStore,
    node: &ast::Node,
    source_file: Option<&ast::SourceFile>,
    flags: GetLiteralTextFlags,
) -> String {
    if let Some(source_file) = source_file {
        if can_use_original_text(store, node, flags) {
            let source_text =
                scanner::get_source_text_of_node_from_source_file(source_file, node, false);
            if store.kind(*node) != ast::Kind::StringLiteral
                || store
                    .token_flags(*node)
                    .is_some_and(|flags| flags.contains(ast::TokenFlags::SINGLE_QUOTE))
                    == source_text.starts_with('\'')
            {
                return source_text;
            }
        }
    }

    match store.kind(*node) {
        ast::Kind::StringLiteral => {
            let quote_char = if store
                .token_flags(*node)
                .is_some_and(|flags| flags.contains(ast::TokenFlags::SINGLE_QUOTE))
            {
                QuoteChar::SingleQuote
            } else {
                QuoteChar::DoubleQuote
            };

            let text = store.text(*node);
            let mut b = String::with_capacity(text.len() + 2);
            b.push(quote_char.as_char());
            escape_string_worker(&text, quote_char, flags, &mut b);
            b.push(quote_char.as_char());
            b
        }

        ast::Kind::NoSubstitutionTemplateLiteral
        | ast::Kind::TemplateHead
        | ast::Kind::TemplateMiddle
        | ast::Kind::TemplateTail => {
            let text = store.text(*node);
            let raw_text = store.raw_text(*node).unwrap_or_default();
            let raw = !raw_text.is_empty() || text.is_empty();
            let text_len = if raw { raw_text.len() } else { text.len() };

            let mut b = String::with_capacity(
                match store.kind(*node) {
                    ast::Kind::NoSubstitutionTemplateLiteral | ast::Kind::TemplateTail => 2,
                    ast::Kind::TemplateHead | ast::Kind::TemplateMiddle => 3,
                    _ => 0,
                } + text_len,
            );

            match store.kind(*node) {
                ast::Kind::NoSubstitutionTemplateLiteral | ast::Kind::TemplateHead => b.push('`'),
                ast::Kind::TemplateMiddle | ast::Kind::TemplateTail => b.push('}'),
                _ => {}
            }

            if !raw_text.is_empty() || text.is_empty() {
                b.push_str(&raw_text);
            } else {
                escape_string_worker(&text, QuoteChar::Backtick, flags, &mut b);
            }

            match store.kind(*node) {
                ast::Kind::NoSubstitutionTemplateLiteral => b.push('`'),
                ast::Kind::TemplateHead | ast::Kind::TemplateMiddle => b.push_str("${"),
                ast::Kind::TemplateTail => b.push('`'),
                _ => {}
            }

            b
        }

        ast::Kind::NumericLiteral | ast::Kind::BigIntLiteral => store.text(*node),

        ast::Kind::RegularExpressionLiteral => {
            if flags.contains(GetLiteralTextFlags::TERMINATE_UNTERMINATED_LITERALS)
                && ast::is_unterminated_literal(store, *node)
            {
                let text = store.text(*node);
                let mut b = String::with_capacity(text.len() + 2);
                b.push_str(&text);
                if text.ends_with('\\') {
                    b.push_str(" /");
                } else {
                    b.push('/');
                }
                return b;
            }
            store.text(*node)
        }

        _ => panic!("Unsupported LiteralLikeNode"),
    }
}

pub(crate) fn skip_synthesized_parentheses(store: &ast::AstStore, node: &ast::Node) -> ast::Node {
    let mut node = *node;
    while store.kind(node) == ast::Kind::ParenthesizedExpression
        && ast::node_is_synthesized(store, node)
    {
        let next = store
            .expression(node)
            .expect("parenthesized expression should have expression");
        node = next;
    }
    node
}

pub(crate) fn is_new_expression_without_arguments(store: &ast::AstStore, node: &ast::Node) -> bool {
    store.kind(*node) == ast::Kind::NewExpression && store.arguments(*node).is_none()
}

pub(crate) fn is_binary_operation(
    store: &ast::AstStore,
    node: &ast::Node,
    token: ast::Kind,
) -> bool {
    let node = ast::skip_partially_emitted_expressions(store, *node);
    store.kind(node) == ast::Kind::BinaryExpression
        && store
            .operator_token(node)
            .is_some_and(|operator| store.kind(operator) == token)
}

pub(crate) fn mixing_binary_operators_requires_parentheses(a: ast::Kind, b: ast::Kind) -> bool {
    if a == ast::Kind::QuestionQuestionToken {
        return b == ast::Kind::AmpersandAmpersandToken || b == ast::Kind::BarBarToken;
    }
    if b == ast::Kind::QuestionQuestionToken {
        return a == ast::Kind::AmpersandAmpersandToken || a == ast::Kind::BarBarToken;
    }
    false
}

pub(crate) fn is_immediately_invoked_function_expression_or_arrow_function(
    store: &ast::AstStore,
    node: &ast::Node,
) -> bool {
    let node = ast::skip_partially_emitted_expressions(store, *node);
    if !ast::is_call_expression(store, node) {
        return false;
    }
    let expr = store
        .expression(node)
        .expect("call expression should have expression");
    let expr = ast::skip_partially_emitted_expressions(store, expr);
    ast::is_function_expression(store, expr) || ast::is_arrow_function(store, expr)
}

pub fn is_file_level_unique_name(
    source_file: &ast::SourceFile,
    name: &str,
    has_global_name: Option<fn(String) -> bool>,
) -> bool {
    if has_global_name.is_some_and(|f| f(name.to_owned())) {
        return false;
    }
    !source_file.identifiers().contains_key(name)
}

pub fn has_leading_hash(text: &str) -> bool {
    !text.is_empty() && text.as_bytes()[0] == b'#'
}

pub fn remove_leading_hash(text: &str) -> String {
    if has_leading_hash(text) {
        text[1..].to_owned()
    } else {
        text.to_owned()
    }
}

pub fn ensure_leading_hash(text: &str) -> String {
    if has_leading_hash(text) {
        text.to_owned()
    } else {
        format!("#{text}")
    }
}

pub fn format_generated_name(private_name: bool, prefix: &str, base: &str, suffix: &str) -> String {
    let name =
        remove_leading_hash(prefix) + &remove_leading_hash(base) + &remove_leading_hash(suffix);
    if private_name {
        ensure_leading_hash(&name)
    } else {
        name
    }
}

fn is_ascii_word_character(ch: char) -> bool {
    stringutil::is_ascii_letter(ch) || stringutil::is_digit(ch) || ch == '_'
}

pub fn make_identifier_from_module_name(module_name: &str) -> String {
    let module_name = tspath::get_base_file_name(module_name);
    let mut builder = String::new();
    let mut start = 0;
    let mut pos = 0;
    while pos < module_name.len() {
        let ch = module_name.as_bytes()[pos] as char;
        if pos == 0 && stringutil::is_digit(ch) {
            builder.push('_');
        } else if !is_ascii_word_character(ch) {
            if start < pos {
                builder.push_str(&module_name[start..pos]);
            }
            builder.push('_');
            start = pos + 1;
        }
        pos += 1;
    }
    if start < pos {
        builder.push_str(&module_name[start..pos]);
    }
    builder
}

fn skip_white_space_single_line(text: &str, pos: &mut usize) {
    while *pos < text.len() {
        let ch = text[*pos..].chars().next().unwrap();
        if !stringutil::is_white_space_single_line(ch) {
            break;
        }
        *pos += ch.len_utf8();
    }
}

fn match_white_space_single_line(text: &str, pos: &mut usize) -> bool {
    let start_pos = *pos;
    skip_white_space_single_line(text, pos);
    *pos != start_pos
}

fn match_rune(text: &str, pos: &mut usize, expected: char) -> bool {
    let Some(ch) = text[*pos..].chars().next() else {
        return false;
    };
    if ch == expected {
        *pos += ch.len_utf8();
        return true;
    }
    false
}

fn match_string(text: &str, pos: &mut usize, expected: &str) -> bool {
    let mut text_pos = *pos;
    let mut expected_pos = 0;
    while expected_pos < expected.len() {
        if text_pos >= text.len() {
            return false;
        }
        let expected_rune = expected[expected_pos..].chars().next().unwrap();
        if !match_rune(text, &mut text_pos, expected_rune) {
            return false;
        }
        expected_pos += expected_rune.len_utf8();
    }
    *pos = text_pos;
    true
}

fn match_quoted_string(text: &str, pos: &mut usize) -> bool {
    let mut text_pos = *pos;
    let quote_char = if match_rune(text, &mut text_pos, '\'') {
        '\''
    } else if match_rune(text, &mut text_pos, '"') {
        '"'
    } else {
        return false;
    };
    while text_pos < text.len() {
        let ch = text[text_pos..].chars().next().unwrap();
        text_pos += ch.len_utf8();
        if ch == quote_char {
            *pos = text_pos;
            return true;
        }
    }
    false
}

// /// <reference path="..." />
// /// <reference types="..." />
// /// <reference lib="..." />
// /// <reference no-default-lib="..." />
// /// <amd-dependency path="..." />
// /// <amd-module />
pub fn is_recognized_triple_slash_comment(text: &str, comment_range: ast::CommentRange) -> bool {
    if comment_range.kind == ast::Kind::SingleLineCommentTrivia
        && comment_range.len() > 2
        && text.as_bytes()[comment_range.pos() as usize + 1] == b'/'
        && text.as_bytes()[comment_range.pos() as usize + 2] == b'/'
    {
        let text = &text[(comment_range.pos() + 3) as usize..comment_range.end() as usize];
        let mut pos = 0usize;
        skip_white_space_single_line(text, &mut pos);
        if !match_rune(text, &mut pos, '<') {
            return false;
        }
        if match_string(text, &mut pos, "reference") {
            if !match_white_space_single_line(text, &mut pos) {
                return false;
            }
            if !match_string(text, &mut pos, "path")
                && !match_string(text, &mut pos, "types")
                && !match_string(text, &mut pos, "lib")
                && !match_string(text, &mut pos, "no-default-lib")
            {
                return false;
            }
            skip_white_space_single_line(text, &mut pos);
            if !match_rune(text, &mut pos, '=') {
                return false;
            }
            skip_white_space_single_line(text, &mut pos);
            if !match_quoted_string(text, &mut pos) {
                return false;
            }
        } else if match_string(text, &mut pos, "amd-dependency") {
            if !match_white_space_single_line(text, &mut pos) {
                return false;
            }
            if !match_string(text, &mut pos, "path") {
                return false;
            }
            skip_white_space_single_line(text, &mut pos);
            if !match_rune(text, &mut pos, '=') {
                return false;
            }
            skip_white_space_single_line(text, &mut pos);
            if !match_quoted_string(text, &mut pos) {
                return false;
            }
        } else if match_string(text, &mut pos, "amd-module") {
            skip_white_space_single_line(text, &mut pos);
        } else {
            return false;
        }
        return text[pos..].contains("/>");
    }

    false
}

pub fn is_pinned_comment(text: &str, comment: ast::CommentRange) -> bool {
    comment.kind == ast::Kind::MultiLineCommentTrivia
        && comment.len() > 5
        && text.as_bytes()[comment.pos() as usize + 2] == b'!'
}

pub fn is_jsdoc_like_text(text: &str, comment: ast::CommentRange) -> bool {
    comment.kind == ast::Kind::MultiLineCommentTrivia
        && comment.len() > 5
        && text.as_bytes()[comment.pos() as usize + 2] == b'*'
        && text.as_bytes()[comment.pos() as usize + 3] != b'/'
}

pub(crate) fn calculate_indent(text: &str, mut pos: usize, end: usize) -> i32 {
    let mut current_line_indent = 0;
    let indent_size = crate::get_default_indent_size();
    while pos < end {
        let ch = text[pos..].chars().next().unwrap();
        if !stringutil::is_white_space_single_line(ch) {
            break;
        }
        if ch == '\t' {
            current_line_indent += indent_size - (current_line_indent % indent_size);
        } else {
            current_line_indent += 1;
        }
        pos += ch.len_utf8();
    }

    current_line_indent
}

// lineCharacterCache provides cached line/character lookups for a source file,
// optimized for monotonically increasing positions (e.g., during source map emit).
//
// When positions increase within the same line, only the delta between the last
// position and the new position needs to be scanned for UTF-16 code unit counts,
// turning what would be O(n²) into O(n) for long lines.
//
// Character offsets are measured in UTF-16 code units per the source map specification.
#[derive(Clone, Default)]
pub struct LineCharacterCache {
    line_map: Arc<[core::TextPos]>,
    text: String,
    cached_line: usize,
    cached_pos: i32,
    cached_char: core::UTF16Offset,
    has_cached: bool,
}

pub(crate) fn new_line_character_cache(source: &dyn sourcemap::Source) -> LineCharacterCache {
    LineCharacterCache {
        line_map: source.ecma_line_map(),
        text: source.text(),
        ..Default::default()
    }
}

impl LineCharacterCache {
    // getLineAndCharacter returns the 0-based line number and UTF-16 code unit
    // offset from the start of that line for the given byte position.
    pub(crate) fn get_line_and_character(&mut self, pos: i32) -> (usize, core::UTF16Offset) {
        let line = scanner::compute_line_of_position(&self.line_map, pos);
        let character = if self.has_cached && line == self.cached_line && pos >= self.cached_pos {
            self.cached_char + core::utf16_len(&self.text[self.cached_pos as usize..pos as usize])
        } else {
            core::utf16_len(&self.text[self.line_map[line] as usize..pos as usize])
        };
        self.cached_line = line;
        self.cached_pos = pos;
        self.cached_char = character;
        self.has_cached = true;
        (line, character)
    }
}
