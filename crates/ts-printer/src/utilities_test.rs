use ts_ast as ast;
use ts_core as core;

use crate::utilities::{
    escape_jsx_attribute_string, escape_non_ascii_string, is_recognized_triple_slash_comment,
};
use crate::{QuoteChar, escape_string};

#[test]
fn test_escape_string() {
    let data = [
        ("", QuoteChar::DoubleQuote, ""),
        ("abc", QuoteChar::DoubleQuote, "abc"),
        ("ab\"c", QuoteChar::DoubleQuote, r#"ab\"c"#),
        ("ab\tc", QuoteChar::DoubleQuote, r#"ab\tc"#),
        ("ab\nc", QuoteChar::DoubleQuote, r#"ab\nc"#),
        ("ab'c", QuoteChar::DoubleQuote, "ab'c"),
        ("ab'c", QuoteChar::SingleQuote, r#"ab\'c"#),
        ("ab\"c", QuoteChar::SingleQuote, "ab\"c"),
        ("ab`c", QuoteChar::Backtick, "ab\\`c"),
        ("\u{001f}", QuoteChar::Backtick, r#"\u001F"#),
    ];

    for (s, quote_char, expected) in data {
        let actual = escape_string(s.to_owned(), quote_char);
        assert_eq!(expected, actual);
    }
}

#[test]
fn test_escape_non_ascii_string() {
    let data = [
        ("", QuoteChar::DoubleQuote, ""),
        ("abc", QuoteChar::DoubleQuote, "abc"),
        ("ab\"c", QuoteChar::DoubleQuote, r#"ab\"c"#),
        ("ab\tc", QuoteChar::DoubleQuote, r#"ab\tc"#),
        ("ab\nc", QuoteChar::DoubleQuote, r#"ab\nc"#),
        ("ab'c", QuoteChar::DoubleQuote, "ab'c"),
        ("ab'c", QuoteChar::SingleQuote, r#"ab\'c"#),
        ("ab\"c", QuoteChar::SingleQuote, "ab\"c"),
        ("ab`c", QuoteChar::Backtick, "ab\\`c"),
        ("ab\u{008f}c", QuoteChar::DoubleQuote, r#"ab\u008Fc"#),
        ("𝟘𝟙", QuoteChar::DoubleQuote, r#"\uD835\uDFD8\uD835\uDFD9"#),
    ];

    for (s, quote_char, expected) in data {
        let actual = escape_non_ascii_string(s.to_owned(), quote_char);
        assert_eq!(expected, actual);
    }
}

#[test]
fn test_escape_jsx_attribute_string() {
    let data = [
        ("", QuoteChar::DoubleQuote, ""),
        ("abc", QuoteChar::DoubleQuote, "abc"),
        ("ab\"c", QuoteChar::DoubleQuote, "ab&quot;c"),
        ("ab\tc", QuoteChar::DoubleQuote, "ab&#x9;c"),
        ("ab\nc", QuoteChar::DoubleQuote, "ab&#xA;c"),
        ("ab'c", QuoteChar::DoubleQuote, "ab'c"),
        ("ab'c", QuoteChar::SingleQuote, "ab&apos;c"),
        ("ab\"c", QuoteChar::SingleQuote, "ab\"c"),
        ("ab\u{008f}c", QuoteChar::DoubleQuote, "ab\u{008F}c"),
        ("𝟘𝟙", QuoteChar::DoubleQuote, "𝟘𝟙"),
    ];

    for (s, quote_char, expected) in data {
        let actual = escape_jsx_attribute_string(s.to_owned(), quote_char);
        assert_eq!(expected, actual);
    }
}

#[test]
fn test_is_recognized_triple_slash_comment() {
    let data = [
        ("", ast::Kind::MultiLineCommentTrivia, false),
        ("", ast::Kind::SingleLineCommentTrivia, false),
        ("/a", ast::Kind::Unknown, false),
        ("//", ast::Kind::Unknown, false),
        ("//a", ast::Kind::Unknown, false),
        ("///", ast::Kind::Unknown, false),
        ("///a", ast::Kind::Unknown, false),
        ("///<reference path=\"foo\" />", ast::Kind::Unknown, true),
        ("///<reference types=\"foo\" />", ast::Kind::Unknown, true),
        ("///<reference lib=\"foo\" />", ast::Kind::Unknown, true),
        (
            "///<reference no-default-lib=\"foo\" />",
            ast::Kind::Unknown,
            true,
        ),
        (
            "///<amd-dependency path=\"foo\" />",
            ast::Kind::Unknown,
            true,
        ),
        ("///<amd-module />", ast::Kind::Unknown, true),
        ("/// <reference path=\"foo\" />", ast::Kind::Unknown, true),
        ("/// <reference types=\"foo\" />", ast::Kind::Unknown, true),
        ("/// <reference lib=\"foo\" />", ast::Kind::Unknown, true),
        (
            "/// <reference no-default-lib=\"foo\" />",
            ast::Kind::Unknown,
            true,
        ),
        (
            "/// <amd-dependency path=\"foo\" />",
            ast::Kind::Unknown,
            true,
        ),
        ("/// <amd-module />", ast::Kind::Unknown, true),
        ("/// <reference path=\"foo\"/>", ast::Kind::Unknown, true),
        ("/// <reference types=\"foo\"/>", ast::Kind::Unknown, true),
        ("/// <reference lib=\"foo\"/>", ast::Kind::Unknown, true),
        (
            "/// <reference no-default-lib=\"foo\"/>",
            ast::Kind::Unknown,
            true,
        ),
        (
            "/// <amd-dependency path=\"foo\"/>",
            ast::Kind::Unknown,
            true,
        ),
        ("/// <amd-module/>", ast::Kind::Unknown, true),
        ("/// <reference path='foo' />", ast::Kind::Unknown, true),
        ("/// <reference types='foo' />", ast::Kind::Unknown, true),
        ("/// <reference lib='foo' />", ast::Kind::Unknown, true),
        (
            "/// <reference no-default-lib='foo' />",
            ast::Kind::Unknown,
            true,
        ),
        (
            "/// <amd-dependency path='foo' />",
            ast::Kind::Unknown,
            true,
        ),
        ("/// <reference path=\"foo\" />  ", ast::Kind::Unknown, true),
        (
            "/// <reference types=\"foo\" />  ",
            ast::Kind::Unknown,
            true,
        ),
        ("/// <reference lib=\"foo\" />  ", ast::Kind::Unknown, true),
        (
            "/// <reference no-default-lib=\"foo\" />  ",
            ast::Kind::Unknown,
            true,
        ),
        (
            "/// <amd-dependency path=\"foo\" />  ",
            ast::Kind::Unknown,
            true,
        ),
        ("/// <amd-module />  ", ast::Kind::Unknown, true),
        ("/// <foo />", ast::Kind::Unknown, false),
        ("/// <reference />", ast::Kind::Unknown, false),
        ("/// <amd-dependency />", ast::Kind::Unknown, false),
    ];

    for (s, kind, expected) in data {
        let comment_range = if kind == ast::Kind::Unknown {
            ast::CommentRange {
                kind: ast::Kind::SingleLineCommentTrivia,
                text_range: core::new_text_range(0, s.len() as i32),
                has_trailing_new_line: false,
            }
        } else {
            ast::CommentRange {
                kind,
                text_range: core::undefined_text_range(),
                has_trailing_new_line: false,
            }
        };
        let actual = is_recognized_triple_slash_comment(s, comment_range);
        assert_eq!(expected, actual);
    }
}
