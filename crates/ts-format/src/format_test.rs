use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;

use super::*;
use crate::lsutil;

#[test]
fn test_format_no_trailing_space() {
    struct TestCase {
        name: &'static str,
        text: &'static str,
    }

    let test_cases = vec![
        TestCase {
            name: "simple statement without trailing newline",
            text: "1;",
        },
        TestCase {
            name: "function call without trailing newline",
            text: "console.log('hello');",
        },
        TestCase {
            name: "if block on single line",
            text: "if (true) { }",
        },
        TestCase {
            name: "class declaration",
            text: "class A {\n    // Class Contents Go Here\n}",
        },
        TestCase {
            name: "class declaration with trailing newline",
            text: "class A {\n    // Class Contents Go Here\n}\n",
        },
        TestCase {
            name: "empty block",
            text: "if (true) {}",
        },
        TestCase {
            name: "module declaration",
            text: "module M { }",
        },
        TestCase {
            name: "enum declaration",
            text: "enum E { A, B }",
        },
    ];

    for tc in test_cases {
        let ctx = with_format_code_settings(
            Context::new(),
            lsutil::FormatCodeSettings {
                editor_settings: lsutil::EditorSettings {
                    tab_size: 4,
                    indent_size: 4,
                    new_line_character: "\n".to_owned(),
                    convert_tabs_to_spaces: core::TS_TRUE,
                    indent_style: lsutil::IndentStyle::Smart,
                    trim_trailing_whitespace: core::TS_TRUE,
                    ..Default::default()
                },
                ..Default::default()
            },
            "\n".to_owned(),
        );
        let source_file = parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/test.ts".to_owned(),
                path: "/test.ts".to_owned(),
                external_module_indicator_options: Default::default(),
            },
            tc.text.to_owned(),
            core::ScriptKind::TS,
        );
        let edits = format_document(&ctx, &source_file);
        let new_text = super::api_test::apply_bulk_edits(tc.text.to_owned(), edits);
        // Formatting should not add trailing whitespace at end of file
        for (i, line) in new_text.split('\n').enumerate() {
            let trimmed = line.trim_end_matches([' ', '\t']);
            assert_eq!(
                line,
                trimmed,
                "Formatter should not add trailing whitespace on line {} in {}",
                i + 1,
                tc.name
            );
        }
    }
}

#[test]
fn format_document_reindents_multiline_parameters_and_conditions() {
    let ctx = with_format_code_settings(
        Context::new(),
        lsutil::FormatCodeSettings {
            editor_settings: lsutil::EditorSettings {
                tab_size: 4,
                indent_size: 4,
                new_line_character: "\n".to_owned(),
                convert_tabs_to_spaces: core::TS_TRUE,
                indent_style: lsutil::IndentStyle::Smart,
                trim_trailing_whitespace: core::TS_TRUE,
                ..Default::default()
            },
            ..Default::default()
        },
        "\n".to_owned(),
    );
    let text = "class TestClass {\n    private testMethod1(param1: boolean,\n                        param2: boolean) {\n    }\n\n    public testMethod2(a: number, b: number, c: number) {\n        if (a === b) {\n        }\n        else if (a != c &&\n                 a > b &&\n                 b < c) {\n        }\n\n    }\n}";
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/test.ts".to_owned(),
            path: "/test.ts".to_owned(),
            external_module_indicator_options: Default::default(),
        },
        text.to_owned(),
        core::ScriptKind::TS,
    );
    let edits = format_document(&ctx, &source_file);
    let new_text = super::api_test::apply_bulk_edits(text.to_owned(), edits);

    assert!(new_text.contains("\n        param2: boolean) {"));
    assert!(new_text.contains("\n            a > b &&"));
    assert!(new_text.contains("\n            b < c) {"));
}

#[test]
fn format_document_removes_space_after_new_in_construct_signature() {
    let ctx = with_format_code_settings(
        Context::new(),
        lsutil::FormatCodeSettings {
            editor_settings: lsutil::EditorSettings {
                tab_size: 4,
                indent_size: 4,
                new_line_character: "\n".to_owned(),
                convert_tabs_to_spaces: core::TS_TRUE,
                indent_style: lsutil::IndentStyle::Smart,
                trim_trailing_whitespace: core::TS_TRUE,
                ..Default::default()
            },
            ..Default::default()
        },
        "\n".to_owned(),
    );
    let text = "type T = { new (): any; };";
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/test.ts".to_owned(),
            path: "/test.ts".to_owned(),
            external_module_indicator_options: Default::default(),
        },
        text.to_owned(),
        core::ScriptKind::TS,
    );
    let edits = format_document(&ctx, &source_file);
    let new_text = super::api_test::apply_bulk_edits(text.to_owned(), edits);

    assert_eq!("type T = { new(): any; };", new_text);
}
