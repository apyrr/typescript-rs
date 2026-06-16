use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;

use super::*;

#[test]
fn test_get_indentation_for_named_imports_position() {
    let text = "import {\n    type SomeInterface,\n} from \"./exports.js\";";
    // Position 9: \n
    // Position 10: first space of "    type SomeInterface"

    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/test.ts".to_owned(),
            path: "/test.ts".to_owned(),
            external_module_indicator_options: Default::default(),
        },
        text.to_owned(),
        core::ScriptKind::TS,
    );

    let options = crate::lsutil::get_default_format_code_settings();

    // The line that contains "    type SomeInterface" starts at position 9 (the \n).
    // The getAdjustedStartPosition with LeadingTriviaOptionNone returns line start.
    // Let's test at position 9 (start of line containing the specifier)
    let line_start = get_line_start_position_for_position(14, &source_file); // 14 is somewhere in "    type"

    let indent = get_indentation(line_start, &source_file, options, true);
    eprintln!(
        "lineStart={}, text[lineStart:]={:?}",
        line_start,
        &text[line_start as usize..line_start as usize + 10]
    );
    eprintln!("GetIndentation at lineStart {} = {}", line_start, indent);

    assert_eq!(indent, 4, "Expected indentation 4, got {indent}");
}
