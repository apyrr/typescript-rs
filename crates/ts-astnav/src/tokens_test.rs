// package astnav_test

use std::fmt::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use ts_ast as ast;
use ts_astnav::tokens as astnav;
use ts_core as core;
use ts_json as json;
use ts_parser as parser;
use ts_repo as repo;
use ts_testutil::{baseline, jstest};

static TEST_FILES: &[&str] = &["src/services/mapCode.ts"];
static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn test_get_token_at_position() {
    crate::testmain_test::test_main();
    // t.Parallel()
    // repo.SkipIfNoTypeScriptSubmodule(t)
    // jstest.SkipIfNoNodeJS(t)

    baseline_tokens(
        "GetTokenAtPosition",
        false, /*includeEOF*/
        ts_get_tokens_at_positions,
        |file, pos| to_token_info(file, astnav::get_token_at_position(file, pos)),
    );

    baseline_go_tokens_json("GetTokenAtPosition", |file, pos| {
        to_token_info(file, astnav::get_token_at_position(file, pos))
    });

    let file_text = "function foo(x) {\n    const s = /**@type {string}*/(x)\n}";
    let file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/test.js".to_owned(),
            path: "/test.js".into(),
            external_module_indicator_options: Default::default(),
        },
        file_text.to_owned(),
        core::ScriptKind::JS,
    );

    // Position of 'x' inside the parenthesized expression (position 52)
    let position = 52;

    // This should not panic - it previously panicked with:
    // "did not expect KindParenthesizedExpression to have KindIdentifier in its trivia"
    let token = astnav::get_touching_property_name(&file, position)
        .expect("Expected to get a token, got nil");

    // The function may return either the identifier itself or the containing
    // parenthesized expression, depending on how the AST is structured
    assert!(
        token.kind == ast::Kind::Identifier || token.kind == ast::Kind::ParenthesizedExpression,
        "Expected identifier or parenthesized expression, got {:?}",
        token.kind
    );

    // Exact code from the issue report
    let file_text = "function foo(x) {\n    const s = /**@type {string}*/(x)  // Go-to-definition on x causes panic\n}";
    let file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/test.js".to_owned(),
            path: "/test.js".into(),
            external_module_indicator_options: Default::default(),
        },
        file_text.to_owned(),
        core::ScriptKind::JS,
    );

    // Find position of 'x' in the type assertion
    let x_pos = 52; // Position of 'x' in (x)

    // This should not panic
    let token = astnav::get_touching_property_name(&file, x_pos);
    assert!(token.is_some(), "Expected to get a token");

    let file_text = "\n\t\t\tfunction foo() {\n\t\t\t\treturn 0;\n\t\t\t}\n\t\t";
    let file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: "/file.ts".to_owned(),
            path: "/file.ts".into(),
            external_module_indicator_options: Default::default(),
        },
        file_text.to_owned(),
        core::ScriptKind::TS,
    );
    assert!(std::ptr::eq(
        astnav::get_token_at_position(&file, 0).unwrap(),
        astnav::get_token_at_position(&file, 0).unwrap()
    ));
}

#[test]
fn test_get_touching_property_name() {
    crate::testmain_test::test_main();
    // t.Parallel()
    // jstest.SkipIfNoNodeJS(t)
    // repo.SkipIfNoTypeScriptSubmodule(t)

    baseline_tokens(
        "GetTouchingPropertyName",
        false, /*includeEOF*/
        ts_get_touching_property_name,
        |file, pos| to_token_info(file, astnav::get_touching_property_name(file, pos)),
    );

    baseline_go_tokens_json("GetTouchingPropertyName", |file, pos| {
        to_token_info(file, astnav::get_touching_property_name(file, pos))
    });
}

fn baseline_tokens(
    test_name: &str,
    include_eof: bool,
    get_ts_tokens: fn(&str, &[i32]) -> Vec<Option<TokenInfo>>,
    get_go_token: fn(&ast::SourceFile, i32) -> Option<TokenInfo>,
) {
    for file_name in TEST_FILES {
        let file_name = type_script_submodule_path().join(file_name);
        let file_text = std::fs::read_to_string(&file_name).expect("read test file");

        let positions: Vec<i32> = (0..(file_text.len() + core::if_else(include_eof, 1, 0)))
            .map(|pos| pos as i32)
            .collect();
        let ts_tokens = get_ts_tokens(&file_text, &positions);
        let file = parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/file.ts".to_owned(),
                path: "/file.ts".into(),
                external_module_indicator_options: Default::default(),
            },
            file_text.clone(),
            core::ScriptKind::TS,
        );

        let mut output = String::new();
        let mut current_range = core::TextRange::new(0, 0);
        let mut current_diff = TokenDiff::default();

        for (pos, ts_token) in ts_tokens.iter().enumerate() {
            let go_token = get_go_token(&file, pos as i32);
            let diff = TokenDiff {
                go_token,
                ts_token: ts_token.clone(),
            };

            if !diff_equal(&current_diff, &diff) {
                if !tokens_equal(&current_diff.go_token, &current_diff.ts_token) {
                    write_range_diff(&mut output, &file, &current_diff, current_range, pos as i32);
                }
                current_diff = diff;
                current_range = core::TextRange::new(pos as i32, pos as i32);
            }
            current_range = current_range.with_end(pos as i32);
        }

        if !tokens_equal(&current_diff.go_token, &current_diff.ts_token) {
            write_range_diff(
                &mut output,
                &file,
                &current_diff,
                current_range,
                ts_tokens.len() as i32 - 1,
            );
        }

        baseline_run(
            &format!(
                "{}.{}.baseline.txt",
                test_name,
                file_name.file_name().unwrap().to_string_lossy()
            ),
            if output.is_empty() {
                baseline_no_content()
            } else {
                output
            },
            "astnav",
        );
    }
}

#[derive(Clone, Serialize)]
struct TokenRun {
    #[serde(rename = "startPos")]
    start_pos: i32,
    #[serde(rename = "endPos")]
    end_pos: i32,
    kind: String,
    #[serde(rename = "nodePos")]
    node_pos: i32,
    #[serde(rename = "nodeEnd")]
    node_end: i32,
}

fn baseline_go_tokens_json(
    test_name: &str,
    get_go_token: fn(&ast::SourceFile, i32) -> Option<TokenInfo>,
) {
    for file_name in TEST_FILES {
        let file_name = type_script_submodule_path().join(file_name);
        let file_text = std::fs::read_to_string(&file_name).expect("read test file");

        let file = parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/file.ts".to_owned(),
                path: "/file.ts".into(),
                external_module_indicator_options: Default::default(),
            },
            file_text.clone(),
            core::ScriptKind::TS,
        );

        let max_pos = file_text.len() as i32;
        let mut runs: Vec<TokenRun> = Vec::new();
        let mut current: Option<TokenRun> = None;

        for pos in 0..max_pos {
            let token = get_go_token(&file, pos);
            if let (Some(current_run), Some(token)) = (current.as_mut(), token.as_ref()) {
                if current_run.kind == token.kind
                    && current_run.node_pos == token.pos
                    && current_run.node_end == token.end
                {
                    current_run.end_pos = pos;
                    continue;
                }
            }
            if let Some(current_run) = current.take() {
                runs.push(current_run);
            }
            current = token.map(|token| TokenRun {
                start_pos: pos,
                end_pos: pos,
                kind: token.kind,
                node_pos: token.pos,
                node_end: token.end,
            });
        }
        if let Some(current_run) = current {
            runs.push(current_run);
        }

        let output = core::stringify_json(&runs, "", "  ").expect("stringify json");

        baseline_run(
            &format!(
                "{}.{}.baseline.json",
                test_name,
                file_name.file_name().unwrap().to_string_lossy()
            ),
            output,
            "astnav",
        );
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct TokenDiff {
    go_token: Option<TokenInfo>,
    ts_token: Option<TokenInfo>,
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
struct TokenInfo {
    kind: String,
    pos: i32,
    end: i32,
}

fn to_token_info(file: &ast::SourceFile, node: Option<ast::Node>) -> Option<TokenInfo> {
    let node = node?;
    let store = file.store();
    let loc = store.loc(node);
    let mut kind = format!("{:?}", store.kind(node)).replacen("Kind", "", 1);
    if kind == "EndOfFile" {
        kind = "EndOfFileToken".to_owned();
    }
    Some(TokenInfo {
        kind,
        pos: loc.pos(),
        end: loc.end(),
    })
}

fn diff_equal(a: &TokenDiff, b: &TokenDiff) -> bool {
    tokens_equal(&a.go_token, &b.go_token) && tokens_equal(&a.ts_token, &b.ts_token)
}

fn tokens_equal(t1: &Option<TokenInfo>, t2: &Option<TokenInfo>) -> bool {
    t1 == t2
}

fn ts_get_tokens_at_positions(file_text: &str, positions: &[i32]) -> Vec<Option<TokenInfo>> {
    let dir = temp_dir();
    std::fs::write(dir.join("file.ts"), file_text).expect("write file");
    std::fs::write(
        dir.join("positions.json"),
        core::stringify_json(positions, "", "").expect("positions json"),
    )
    .expect("write positions");

    let script = r#"
		import fs from "fs";
		export default (ts) => {
			const positions = JSON.parse(fs.readFileSync("positions.json", "utf8"));
			const fileText = fs.readFileSync("file.ts", "utf8");
			const file = ts.createSourceFile(
				"file.ts",
				fileText,
				ts.ScriptTarget.Latest,
				/*setParentNodes*/ true
			);
			return positions.map(position => {
				let token = ts.getTokenAtPosition(file, position);
				if (token.kind === ts.SyntaxKind.SyntaxList) {
					token = token.parent;
				}
				return {
					kind: ts.Debug.formatSyntaxKind(token.kind),
					pos: token.pos,
					end: token.end,
				};
			});
		};"#;

    eval_node_script_with_ts(script, &dir, "")
}

fn ts_get_touching_property_name(file_text: &str, positions: &[i32]) -> Vec<Option<TokenInfo>> {
    let dir = temp_dir();
    std::fs::write(dir.join("file.ts"), file_text).expect("write file");
    std::fs::write(
        dir.join("positions.json"),
        core::stringify_json(positions, "", "").expect("positions json"),
    )
    .expect("write positions");

    let script = r#"
		import fs from "fs";
		export default (ts) => {
			const positions = JSON.parse(fs.readFileSync("positions.json", "utf8"));
			const fileText = fs.readFileSync("file.ts", "utf8");
			const file = ts.createSourceFile(
				"file.ts",
				fileText,
				ts.ScriptTarget.Latest,
				/*setParentNodes*/ true
			);
			return positions.map(position => {
				let token = ts.getTouchingPropertyName(file, position);
				if (token.kind === ts.SyntaxKind.SyntaxList) {
					token = token.parent;
				}
				return {
					kind: ts.Debug.formatSyntaxKind(token.kind),
					pos: token.pos,
					end: token.end,
				};
			});
		};"#;

    eval_node_script_with_ts(script, &dir, "")
}

fn write_range_diff(
    output: &mut String,
    file: &ast::SourceFile,
    diff: &TokenDiff,
    rng: core::TextRange,
    position: i32,
) {
    let lines = file.ecma_line_map();

    let mut ts_token_pos = position;
    let mut go_token_pos = position;
    let mut ts_token_end = position;
    let mut go_token_end = position;
    if let Some(ts_token) = &diff.ts_token {
        ts_token_pos = ts_token.pos;
        ts_token_end = ts_token.end;
    }
    if let Some(go_token) = &diff.go_token {
        go_token_pos = go_token.pos;
        go_token_end = go_token.end;
    }
    let (ts_start_line, _) = core::position_to_line_and_byte_offset(ts_token_pos as usize, &lines);
    let (ts_end_line, _) = core::position_to_line_and_byte_offset(ts_token_end as usize, &lines);
    let (go_start_line, _) = core::position_to_line_and_byte_offset(go_token_pos as usize, &lines);
    let (go_end_line, _) = core::position_to_line_and_byte_offset(go_token_end as usize, &lines);

    let context_lines = 2usize;
    let start_line = std::cmp::min(ts_start_line, go_start_line);
    let end_line = std::cmp::max(ts_end_line, go_end_line);
    let mut marker_lines = vec![ts_start_line, ts_end_line, go_start_line, go_end_line];
    marker_lines.sort();
    let context_start = start_line.saturating_sub(context_lines);
    let context_end = std::cmp::min(lines.len() - 1, end_line + context_lines);
    let digits = context_end.to_string().len();

    let should_truncate = |line: usize| -> (bool, usize) {
        let index = marker_lines
            .binary_search(&line)
            .unwrap_or_else(|index| index);
        if index == 0 || index == marker_lines.len() {
            return (false, 0);
        }
        let low = marker_lines[index - 1];
        let high = marker_lines[index];
        if line - low > 5 && high - line > 5 {
            return (true, high - 5);
        }
        (false, 0)
    };

    if !output.is_empty() {
        output.push_str("\n\n");
    }

    writeln!(output, "〚Positions: [{}, {}]〛", rng.pos(), rng.end()).unwrap();
    if let Some(ts_token) = &diff.ts_token {
        writeln!(
            output,
            "【TS: {} [{}, {})】",
            ts_token.kind, ts_token_pos, ts_token_end
        )
        .unwrap();
    } else {
        output.push_str("【TS: nil】\n");
    }
    if let Some(go_token) = &diff.go_token {
        writeln!(
            output,
            "《Go: {} [{}, {})》",
            go_token.kind, go_token_pos, go_token_end
        )
        .unwrap();
    } else {
        output.push_str("《Go: nil》\n");
    }
    let mut line = context_start;
    while line <= context_end {
        let (truncate, skip_to) = should_truncate(line);
        if truncate {
            writeln!(
                output,
                "{:width$} │........ {} lines omitted ........",
                "",
                skip_to - line + 1,
                width = digits
            )
            .unwrap();
            line = skip_to;
        }
        write!(output, "{:>width$} │", line + 1, width = digits).unwrap();
        let mut end = file.text().len() as i32 + 1;
        if line < lines.len() - 1 {
            end = lines[line + 1];
        }
        for pos in lines[line]..end {
            if pos == rng.end() + 1 {
                output.push('〛');
            }
            if diff.ts_token.is_some() && pos == ts_token_end {
                output.push('】');
            }
            if diff.go_token.is_some() && pos == go_token_end {
                output.push('》');
            }

            if diff.go_token.is_some() && pos == go_token_pos {
                output.push('《');
            }
            if diff.ts_token.is_some() && pos == ts_token_pos {
                output.push('【');
            }
            if pos == rng.pos() {
                output.push('〚');
            }

            if pos < file.text().len() as i32 {
                output.push(file.text().as_bytes()[pos as usize] as char);
            }
        }
        line += 1;
    }
}

#[test]
fn test_find_preceding_token() {
    crate::testmain_test::test_main();
    // t.Parallel()
    // repo.SkipIfNoTypeScriptSubmodule(t)
    // jstest.SkipIfNoNodeJS(t)

    baseline_tokens(
        "FindPrecedingToken",
        true, /*includeEOF*/
        ts_find_preceding_tokens,
        |file, pos| to_token_info(file, astnav::find_preceding_token(file, pos)),
    );

    baseline_go_tokens_json("FindPrecedingToken", |file, pos| {
        to_token_info(file, astnav::find_preceding_token(file, pos))
    });
}

#[test]
fn test_find_next_token() {
    crate::testmain_test::test_main();
    // t.Parallel()
    // repo.SkipIfNoTypeScriptSubmodule(t)

    baseline_go_tokens_json("FindNextToken", |file, pos| {
        // FindNextToken panics (like Go's assert) when the scanner finds trivia between
        // previousToken.End() and the next syntactic token. Catch those to avoid crashing
        // the baseline generator; those positions will be absent from the baseline.
        std::panic::catch_unwind(|| {
            let token = astnav::get_token_at_position(file, pos).expect("expected token");
            astnav::find_next_token(token, file.as_node(), file)
        })
        .ok()
        .flatten()
        .and_then(|next| to_token_info(file, Some(next)))
    });
}

#[test]
fn test_unit_find_preceding_token() {
    crate::testmain_test::test_main();
    struct TestCase {
        name: &'static str,
        file_content: &'static str,
        position: i32,
        expected_kind: ast::Kind,
    }

    let test_cases = vec![
        TestCase {
            name: "after dot after comments",
            file_content: r#"import {
    CharacterCodes,
    compareStringsCaseInsensitive,
    compareStringsCaseSensitive,
    compareValues,
    Comparison,
    Debug,
    endsWith,
    equateStringsCaseInsensitive,
    equateStringsCaseSensitive,
    GetCanonicalFileName,
    getDeclarationFileExtension,
    getStringComparer,
    identity,
    lastOrUndefined,
    Path,
    some,
    startsWith,
} from "./_namespaces/ts.js";

/**
 * Internally, we represent paths as strings with '/' as the directory separator.
 * When we make system calls (eg: LanguageServiceHost.getDirectory()),
 * we expect the host to correctly handle paths in our specified format.
 *
 * @internal
 */
export const directorySeparator = "/";
/** @internal */
export const altDirectorySeparator = "\\";
const urlSchemeSeparator = "://";
const backslashRegExp = /\\/g;


backslashRegExp.

//Path Tests

/**
 * Determines whether a charCode corresponds to '/' or '\'.
 *
 * @internal
 */
export function isAnyDirectorySeparator(charCode: number): boolean {
    return charCode === CharacterCodes.slash || charCode === CharacterCodes.backslash;
}"#,
            position: 839,
            expected_kind: ast::Kind::DotToken,
        },
        TestCase {
            name: "after comma in parameter list",
            file_content: "takesCb((n, s, ))",
            position: 15,
            expected_kind: ast::Kind::CommaToken,
        },
    ];

    for test_case in test_cases {
        let file = parser::parse_source_file(
            ast::SourceFileParseOptions {
                file_name: "/file.ts".to_owned(),
                path: "/file.ts".into(),
                external_module_indicator_options: Default::default(),
            },
            test_case.file_content.to_owned(),
            core::ScriptKind::TS,
        );
        let token = astnav::find_preceding_token(&file, test_case.position).unwrap();
        assert_eq!(token.kind, test_case.expected_kind, "{}", test_case.name);
    }
}

fn ts_find_preceding_tokens(file_text: &str, positions: &[i32]) -> Vec<Option<TokenInfo>> {
    let dir = temp_dir();
    std::fs::write(dir.join("file.ts"), file_text).expect("write file");
    std::fs::write(
        dir.join("positions.json"),
        core::stringify_json(positions, "", "").expect("positions json"),
    )
    .expect("write positions");

    let script = r#"
		import fs from "fs";
		export default (ts) => {
			const positions = JSON.parse(fs.readFileSync("positions.json", "utf8"));
			const fileText = fs.readFileSync("file.ts", "utf8");
			const file = ts.createSourceFile(
				"file.ts",
				fileText,
				ts.ScriptTarget.Latest,
				/*setParentNodes*/ true
			);
			return positions.map(position => {
				let token = ts.findPrecedingToken(position, file);
				if (token === undefined) {
					return undefined;
				}
				if (token.kind === ts.SyntaxKind.SyntaxList) {
					token = token.parent;
				}
				return {
					kind: ts.Debug.formatSyntaxKind(token.kind),
					pos: token.pos,
					end: token.end,
				};
			});
		};"#;
    eval_node_script_with_ts(script, &dir, "")
}

fn type_script_submodule_path() -> PathBuf {
    repo::type_script_submodule_path().to_path_buf()
}

fn baseline_run(name: &str, output: String, subfolder: &str) {
    baseline::run(
        name,
        &output,
        baseline::Options {
            subfolder: subfolder.to_owned(),
            ..Default::default()
        },
    )
    .expect("baseline run");
}

fn baseline_no_content() -> String {
    baseline::NO_CONTENT.to_owned()
}

fn temp_dir() -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ts_astnav_tokens_test_{}_{}",
        std::process::id(),
        id
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn eval_node_script_with_ts(script: &str, dir: &PathBuf, args: &str) -> Vec<Option<TokenInfo>> {
    let args = if args.is_empty() {
        Vec::new()
    } else {
        vec![args.to_owned()]
    };
    let output = jstest::eval_node_script_with_ts(script, dir, &args)
        .expect("eval node script with TypeScript");
    let mut result = Vec::new();
    json::unmarshal(output.as_bytes(), &mut result, &[]).expect("parse token info json");
    result
}
