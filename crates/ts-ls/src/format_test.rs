use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;

use crate::LanguageService;
use crate::lsutil;

// Test for issue: Panic Handling textDocument/onTypeFormatting
// This reproduces the panic when pressing enter in an empty file
#[test]
fn test_get_formatting_edits_after_keystroke_empty_file() {
    // Create an empty file
    let text = "";
    let source_file = parse_source_file("/index.ts", text);

    // Create language service with nil program (we're only testing the formatting function)
    let lang_service = LanguageService::default();

    // Test formatting after keystroke with newline character at position 0
    let ctx = core::Context::background();
    let options = lsutil::get_default_format_code_settings();

    // This should not panic
    let edits = lang_service.get_formatting_edits_after_keystroke(
        ctx,
        &source_file,
        options,
        0, // position
        "\n",
    );

    // Should return nil or empty edits, not panic
    let _ = edits;
}

// Test with a simple statement
#[test]
fn test_get_formatting_edits_after_keystroke_simple_statement() {
    // Create a file with a simple statement
    let text = "const x = 1";
    let source_file = parse_source_file("/index.ts", text);

    // Create language service with nil program
    let lang_service = LanguageService::default();

    // Test formatting after keystroke with newline character at end of statement
    let ctx = core::Context::background();
    let options = lsutil::get_default_format_code_settings();

    // This should not panic
    let edits = lang_service.get_formatting_edits_after_keystroke(
        ctx,
        &source_file,
        options,
        text.len() as i32, // position at end of file
        "\n",
    );

    // Should return nil or empty edits, not panic
    let _ = edits;
}

#[test]
fn test_format_on_semicolon_indents_statement_in_arrow_function_body() {
    let text = r#"class C2 {
    eventEmitter: any;
    constructor() {
        this.eventEmitter.on(5, (msg) => {
console.log;
        });
    }
}"#;
    let position = text.find("console.log;").unwrap() as i32 + "console.log;".len() as i32;
    let actual = apply_formatting_edits_after_keystroke(text, position, ";");
    assert_eq!(
        actual,
        r#"class C2 {
    eventEmitter: any;
    constructor() {
        this.eventEmitter.on(5, (msg) => {
            console.log;
        });
    }
}"#
    );
}

#[test]
fn test_format_on_enter_respects_control_block_new_line_option() {
    let text = "if(true) {\n}\nif(false){\n}";
    let position = text.find("{\n}").unwrap() as i32 + 2;
    let mut options = lsutil::get_default_format_code_settings();
    options.place_open_brace_on_new_line_for_control_blocks = core::TSTrue;
    let actual = apply_formatting_edits_after_keystroke_with_options(text, position, "\n", options);
    assert_eq!(actual, "if (true)\n{\n}\nif(false){\n}",);
}

fn apply_formatting_edits_after_keystroke(text: &str, position: i32, key: &str) -> String {
    apply_formatting_edits_after_keystroke_with_options(
        text,
        position,
        key,
        lsutil::get_default_format_code_settings(),
    )
}

fn apply_formatting_edits_after_keystroke_with_options(
    text: &str,
    position: i32,
    key: &str,
    options: lsutil::FormatCodeSettings,
) -> String {
    let source_file = parse_source_file("/index.ts", text);
    let lang_service = LanguageService::default();
    let edits = lang_service.get_formatting_edits_after_keystroke(
        core::Context::background(),
        &source_file,
        options,
        position,
        key,
    );
    let mut actual = text.to_string();
    for edit in edits.into_iter().rev() {
        actual.replace_range(
            edit.text_range.pos() as usize..edit.text_range.end() as usize,
            &edit.new_text,
        );
    }
    actual
}

fn parse_source_file(file_name: &str, text: &str) -> ast::SourceFile {
    parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: file_name.to_string(),
            path: file_name.to_string(),
            ..Default::default()
        },
        text.to_string(),
        core::ScriptKind::TS,
    )
}

// Test for issue: Crash in range formatting when requested on a line that is different from the containing function
// This reproduces the panic when formatting a range inside a function body
#[test]
fn test_get_formatting_edits_for_range_function_body() {
    struct TestCase {
        name: &'static str,
        text: &'static str,
        start_pos: i32,
        end_pos: i32,
    }

    let test_cases = vec![
        TestCase {
            name: "return statement in function",
            text: "function foo() {\n    return (1  + 2);\n}",
            start_pos: 21, // Start of "return"
            end_pos: 38,   // End of ");"
        },
        TestCase {
            name: "function with newline after keyword",
            text: "function\nf() {\n}",
            start_pos: 9, // After "function\n"
            end_pos: 13,  // Inside or after function
        },
        TestCase {
            name: "empty function body",
            text: "function f() {\n  \n}",
            start_pos: 15, // Inside body
            end_pos: 17,   // Inside body
        },
        TestCase {
            name: "after function closing brace",
            text: "function f() {\n}",
            start_pos: 15, // After closing brace
            end_pos: 15,
        },
    ];

    for test_case in test_cases {
        let source_file = parse_source_file("/test.ts", test_case.text);

        let lang_service = LanguageService::default();
        let ctx = core::Context::background();
        let options = lsutil::get_default_format_code_settings();

        // This should not panic
        let edits = lang_service.get_formatting_edits_for_range(
            ctx,
            &source_file,
            options,
            core::new_text_range(test_case.start_pos, test_case.end_pos),
        );

        // Should not panic
        let _ = (&test_case.name, edits); // Just ensuring no panic
    }
}
