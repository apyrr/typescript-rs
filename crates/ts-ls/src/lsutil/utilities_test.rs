use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;

use super::probably_uses_semicolons;

fn parse_ts(text: &str) -> ast::SourceFile {
    parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/test.ts".to_string(),
            path: "/test.ts".to_string(),
            external_module_indicator_options: Default::default(),
        },
        text.to_string(),
        core::ScriptKind::TS,
    )
}

#[test]
fn test_probably_uses_semicolons() {
    struct TestCase {
        name: &'static str,
        src: &'static str,
        want: bool,
    }

    let tests = [
        TestCase {
            name: "mixed semicolons and asi favors semicolons when ratio exceeds one fifth",
            // First five observations: 2 with semicolon, 3 without. Real ratio 2/3 > 1/5.
            // Integer division bug compared against 1/5==0 and used with/without as ints,
            // so the old check was effectively (with/without) > 0, which failed here.
            src: "let a = 1;\nlet b = 2;\nlet c = 3\nlet d = 4\nlet e = 5\n",
            want: true,
        },
        TestCase {
            name: "consistent asi with no semicolons",
            src: "let a = 1\nlet b = 2\nlet c = 3\n",
            want: false,
        },
        TestCase {
            name: "consistent semicolons",
            src: "let a = 1;\nlet b = 2;\nlet c = 3;\n",
            want: true,
        },
    ];

    for test in tests {
        let file = parse_ts(test.src);
        let got = probably_uses_semicolons(&file);
        assert_eq!(got, test.want, "{}", test.name);
    }
}
