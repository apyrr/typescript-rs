use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;

use super::*;
use crate::lsutil;

fn test_context(convert_tabs_to_spaces: core::Tristate, base_indent_size: i32) -> Context {
    with_format_code_settings(
        Context::new(),
        lsutil::FormatCodeSettings {
            editor_settings: lsutil::EditorSettings {
                tab_size: 4,
                indent_size: 4,
                base_indent_size,
                new_line_character: "\n".to_owned(),
                convert_tabs_to_spaces,
                indent_style: lsutil::IndentStyle::Smart,
                trim_trailing_whitespace: core::TS_TRUE,
            },
            insert_space_before_type_annotation: core::TS_TRUE,
            ..Default::default()
        },
        "\n".to_owned(),
    )
}

fn format_text(
    ctx: &Context,
    file_name: &str,
    text: &str,
    script_kind: core::ScriptKind,
) -> String {
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: file_name.to_owned(),
            path: file_name.to_owned(),
            external_module_indicator_options: Default::default(),
        },
        text.to_owned(),
        script_kind,
    );
    let edits = format_document(ctx, &source_file);
    super::api_test::apply_bulk_edits(text.to_owned(), edits)
}

#[test]
fn test_comment_formatting() {
    let ctx = test_context(core::TS_TRUE, 4);

    // Original code that causes the bug
    let original_text = "class C {\n    /**\n     *\n    */\n    async x() {}\n}";

    // Apply formatting once
    let first_formatted = format_text(&ctx, "/test.ts", original_text, core::ScriptKind::TS);

    // Check that the asterisk is not corrupted
    assert!(
        !first_formatted.contains("*/\n   /"),
        "should not corrupt */ to /"
    );
    assert!(first_formatted.contains("*/"), "should preserve */ token");
    assert!(
        first_formatted.contains("async"),
        "should preserve async keyword"
    );

    // Apply formatting a second time to test stability
    let second_formatted = format_text(&ctx, "/test.ts", &first_formatted, core::ScriptKind::TS);

    // Check that second formatting doesn't introduce corruption
    assert!(
        !second_formatted.contains(" sync x()"),
        "should not corrupt async to sync"
    );
    assert!(
        second_formatted.contains("async"),
        "should preserve async keyword on second pass"
    );

    let tab_ctx = test_context(core::TS_FALSE, 0);
    // Original code with tab indentation (tabs represented as \t)
    let original_text = "class Foo {\n\t/**\n\t * @param {string} argument - This is a param description.\n\t */\n\texample(argument) {\nconsole.log(argument);\n\t}\n}";
    let formatted = format_text(&tab_ctx, "/test.ts", original_text, core::ScriptKind::TS);
    // Check that tabs come before spaces (not spaces before tabs)
    // The comment lines should have format: tab followed by space and asterisk
    // NOT: space followed by tab and asterisk
    assert!(
        !formatted.contains(" \t*"),
        "should not have space before tab before asterisk"
    );
    assert!(
        formatted.contains("\t *"),
        "should have tab before space before asterisk"
    );
    // Verify console.log is properly indented with tabs
    assert!(
        formatted.contains("\t\tconsole.log"),
        "console.log should be indented with two tabs"
    );

    // Original code with proper indentation
    let original_text = "console.log(\n\t\"a\",\n\t// the second arg\n\t\"b\"\n);";
    let formatted = format_text(&tab_ctx, "/test.ts", original_text, core::ScriptKind::TS);
    // The comment should remain indented with a tab
    assert!(
        formatted.contains("\t// the second arg"),
        "comment should be indented with tab"
    );
    // The comment should not lose its indentation
    assert!(
        !formatted.contains("\n// the second arg"),
        "comment should not lose indentation"
    );

    // Original code with proper indentation
    let original_text = "foo\n\t.bar()\n\t// A second call\n\t.baz();";
    let formatted = format_text(&tab_ctx, "/test.ts", original_text, core::ScriptKind::TS);
    // The comment should remain indented
    assert!(
        formatted.contains("\t// A second call") || formatted.contains("   // A second call"),
        "comment should be indented"
    );
    // The comment should not lose its indentation
    assert!(
        !formatted.contains("\n// A second call"),
        "comment should not lose indentation"
    );

    // Regression test for issue #1928 - panic when formatting chained method call with comment
    // This code previously caused a panic with "strings: negative Repeat count"
    // because tokenIndentation was -1 and was being used directly for indentation
    let formatted = format_text(&tab_ctx, "/test.ts", original_text, core::ScriptKind::TS);
    // Verify the comment maintains proper indentation and doesn't lose it
    assert!(
        formatted.contains("\t// A second call") || formatted.contains("   // A second call"),
        "comment should be indented"
    );
    assert!(
        !formatted.contains("\n// A second call"),
        "comment should not be at column 0"
    );

    let simple_ctx = with_format_code_settings(
        Context::new(),
        lsutil::FormatCodeSettings {
            editor_settings: lsutil::EditorSettings {
                tab_size: 4,
                indent_size: 4,
                base_indent_size: 0,
                new_line_character: "\n".to_owned(),
                convert_tabs_to_spaces: core::TS_FALSE,
                indent_style: lsutil::IndentStyle::Smart,
                trim_trailing_whitespace: core::TS_TRUE,
            },
            insert_space_before_type_annotation: core::TS_UNKNOWN,
            ..Default::default()
        },
        "\n".to_owned(),
    );

    let original_text = "document.addEventListener('DOMContentLoaded', () => {\n    /** @type {NodeListOf<HTMLSpanElement>} */\n    const elements = document.querySelectorAll('.test')\n});";
    let formatted = format_text(&simple_ctx, "/test.js", original_text, core::ScriptKind::JS);
    assert!(!formatted.is_empty(), "formatted text should not be empty");

    let original_text = "document.addEventListener('DOMContentLoaded', () => {\n    // a comment\n    const x = 1\n});";
    let formatted = format_text(&simple_ctx, "/test.ts", original_text, core::ScriptKind::TS);
    assert!(!formatted.is_empty(), "formatted text should not be empty");
}

#[test]
fn test_slice_bounds_panic() {
    let ctx = test_context(core::TS_TRUE, 4);

    // Code from the issue that causes slice bounds panic
    let original_text = "const _enableDisposeWithListenerWarning = false\n\t// || Boolean(\"TRUE\") // causes a linter warning so that it cannot be pushed\n\t;\n";

    // This should not panic
    let formatted = format_text(&ctx, "/test.ts", original_text, core::ScriptKind::TS);

    // Basic sanity checks
    assert!(!formatted.is_empty(), "formatted text should not be empty");
    assert!(
        formatted.contains("_enableDisposeWithListenerWarning"),
        "should preserve variable name"
    );
}
