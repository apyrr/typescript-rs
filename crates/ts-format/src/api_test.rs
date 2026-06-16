use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;
use ts_printer as printer;
use ts_repo as repo;

use super::*;
use crate::lsutil;

pub(super) fn apply_bulk_edits(text: String, edits: Vec<core::TextChange>) -> String {
    let mut b = String::with_capacity(text.len());
    let mut last_end = 0;
    for e in edits {
        let start = e.text_range.pos();
        if start != last_end {
            b.push_str(&text[last_end as usize..e.text_range.pos() as usize]);
        }
        b.push_str(&e.new_text);

        last_end = e.text_range.end();
    }
    b.push_str(&text[last_end as usize..]);

    b
}

#[test]
fn test_format() {
    let ctx = with_format_code_settings(
        Context::new(),
        lsutil::FormatCodeSettings {
            editor_settings: lsutil::EditorSettings {
                tab_size: 4,
                indent_size: 4,
                base_indent_size: 4,
                new_line_character: "\n".to_owned(),
                convert_tabs_to_spaces: core::TS_TRUE,
                indent_style: lsutil::IndentStyle::Smart,
                trim_trailing_whitespace: core::TS_TRUE,
            },
            insert_space_before_type_annotation: core::TS_TRUE,
            ..Default::default()
        },
        "\n".to_owned(),
    );
    if repo::skip_if_no_type_script_submodule() {
        return;
    }
    let file_path = repo::type_script_submodule_path().join("src/compiler/checker.ts");
    let text = std::fs::read_to_string(file_path).unwrap();
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/checker.ts".to_owned(),
            path: "/checker.ts".to_owned(),
            external_module_indicator_options: Default::default(),
        },
        text.clone(),
        core::ScriptKind::TS,
    );
    let edits = format_document(&ctx, &source_file);
    let new_text = apply_bulk_edits(text.clone(), edits);
    assert!(!new_text.is_empty());
    assert_ne!(text, new_text);
}

#[expect(
    dead_code,
    reason = "manual benchmark helper is not called by normal tests"
)]
fn benchmark_format() {
    let ctx = with_format_code_settings(
        Context::new(),
        lsutil::FormatCodeSettings {
            editor_settings: lsutil::EditorSettings {
                tab_size: 4,
                indent_size: 4,
                base_indent_size: 4,
                new_line_character: "\n".to_owned(),
                convert_tabs_to_spaces: core::TS_TRUE,
                indent_style: lsutil::IndentStyle::Smart,
                trim_trailing_whitespace: core::TS_TRUE,
            },
            insert_space_before_type_annotation: core::TS_TRUE,
            ..Default::default()
        },
        "\n".to_owned(),
    );
    if repo::skip_if_no_type_script_submodule() {
        return;
    }
    let file_path = repo::type_script_submodule_path().join("src/compiler/checker.ts");
    let text = std::fs::read_to_string(file_path).unwrap();
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/checker.ts".to_owned(),
            path: "/checker.ts".to_owned(),
            external_module_indicator_options: Default::default(),
        },
        text.clone(),
        core::ScriptKind::TS,
    );

    for _ in 0..1 {
        let edits = format_document(&ctx, &source_file);
        let new_text = apply_bulk_edits(text.clone(), edits);
        assert!(!new_text.is_empty());
    }

    for _ in 0..1 {
        let edits = format_document(&ctx, &source_file);
        assert!(!edits.is_empty());
    }

    let mut p = printer::new_printer(
        printer::PrinterOptions::default(),
        printer::PrintHandlers::default(),
        Some(printer::new_emit_context()),
    );
    for _ in 0..1 {
        let new_text = p.emit_source_file(&source_file);
        assert!(!new_text.is_empty());
    }
}
