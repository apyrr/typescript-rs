use std::collections::HashMap;
use std::time::SystemTime;

use ts_vfs::vfstest::{self, IntoMapFile};

use super::FileMap;
use super::runner::{TestingT, TscEdit, TscInput, no_change};
use super::sys::TestSys;

struct RustTestingT;

impl TestingT for RustTestingT {
    fn helper(&mut self) {}

    fn run(&mut self, _name: &str, f: &mut dyn FnMut(&mut dyn TestingT)) {
        f(self);
    }

    fn parallel(&mut self) {}

    fn errorf(&mut self, message: &str) {
        panic!("{message}");
    }
}

fn tsc_input(sub_scenario: &str, files: FileMap, command_line_args: Vec<&str>) -> TscInput {
    TscInput {
        sub_scenario: sub_scenario.to_owned(),
        command_line_args: command_line_args.into_iter().map(str::to_owned).collect(),
        files,
        cwd: String::new(),
        edits: Vec::new(),
        env: HashMap::new(),
        ignore_case: false,
        windows_style_root: String::new(),
    }
}

fn tsc_input_with_edits(
    sub_scenario: &str,
    files: FileMap,
    command_line_args: Vec<&str>,
    edits: Vec<TscEdit>,
) -> TscInput {
    let mut input = tsc_input(sub_scenario, files, command_line_args);
    input.edits = edits;
    input
}

fn file_map(files: &[(&str, &str)]) -> FileMap {
    files
        .iter()
        .map(|(path, content)| {
            (
                (*path).to_owned(),
                (*content).into_map_file(SystemTime::UNIX_EPOCH),
            )
        })
        .collect()
}

fn new_tsc_edit(name: &str, edit: fn(&mut TestSys)) -> TscEdit {
    TscEdit {
        caption: name.to_owned(),
        command_line_args: None,
        edit: Some(edit),
        expected_diff: String::new(),
    }
}

fn tsc_edit_with_diff(name: &str, expected_diff: &str, edit: fn(&mut TestSys)) -> TscEdit {
    TscEdit {
        caption: name.to_owned(),
        command_line_args: None,
        edit: Some(edit),
        expected_diff: expected_diff.to_owned(),
    }
}

#[test]
fn test_watch() {
    let mut t = RustTestingT;
    t.parallel();

    let mut symlink_files = file_map(&[
        (
            "/home/src/workspaces/project/index.ts",
            r#"import { shared } from "./link";"#,
        ),
        (
            "/home/src/workspaces/shared/index.ts",
            r#"export const shared = "v1";"#,
        ),
        ("/home/src/workspaces/project/tsconfig.json", "{}"),
    ]);
    symlink_files.insert(
        "/home/src/workspaces/project/link.ts".to_owned(),
        vfstest::symlink("/home/src/workspaces/shared/index.ts"),
    );

    let test_cases = vec![
        tsc_input(
            "watch with no tsconfig",
            file_map(&[("/home/src/workspaces/project/index.ts", "")]),
            vec!["index.ts", "--watch"],
        ),
        tsc_input(
            "watch with tsconfig and incremental",
            file_map(&[
                ("/home/src/workspaces/project/index.ts", ""),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch", "--incremental"],
        ),
        tsc_input_with_edits(
            "watch skips build when no files change",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    "const x: number = 1;",
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![no_change()],
        ),
        tsc_input_with_edits(
            "watch rebuilds when file is modified",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    "const x: number = 1;",
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("modify file", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/index.ts",
                    "const x: number = 2;",
                );
            })],
        ),
        tsc_input_with_edits(
            "watch rebuilds when source file is deleted",
            file_map(&[
                (
                    "/home/src/workspaces/project/a.ts",
                    r#"import { b } from "./b";"#,
                ),
                ("/home/src/workspaces/project/b.ts", "export const b = 1;"),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![tsc_edit_with_diff(
                "delete imported file",
                "incremental resolves to .js output from prior build (TS7016) while clean build cannot find module at all (TS2307)",
                |sys| sys.remove_no_error("/home/src/workspaces/project/b.ts"),
            )],
        ),
        tsc_input_with_edits(
            "watch detects new file resolving failed import",
            file_map(&[
                (
                    "/home/src/workspaces/project/a.ts",
                    r#"import { b } from "./b";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("create missing file", |sys| {
                sys.write_file_no_error("/home/src/workspaces/project/b.ts", "export const b = 1;");
            })],
        ),
        tsc_input_with_edits(
            "watch detects imported file added in new directory",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { util } from "./lib/util";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("create directory and imported file", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/lib/util.ts",
                    r#"export const util = "hello";"#,
                );
            })],
        ),
        tsc_input_with_edits(
            "watch detects imported directory removed",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { util } from "./lib/util";"#,
                ),
                (
                    "/home/src/workspaces/project/lib/util.ts",
                    r#"export const util = "hello";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![tsc_edit_with_diff(
                "remove directory with imported file",
                "incremental resolves to .js output from prior build (TS7016) while clean build cannot find module at all (TS2307)",
                |sys| sys.remove_no_error("/home/src/workspaces/project/lib/util.ts"),
            )],
        ),
        tsc_input_with_edits(
            "watch detects import path restructured",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { util } from "./lib/util";"#,
                ),
                (
                    "/home/src/workspaces/project/lib/util.ts",
                    r#"export const util = "v1";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "move file to new path and update import",
                |sys| {
                    sys.remove_no_error("/home/src/workspaces/project/lib/util.ts");
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/src/util.ts",
                        r#"export const util = "v2";"#,
                    );
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/index.ts",
                        r#"import { util } from "./src/util";"#,
                    );
                },
            )],
        ),
        tsc_input_with_edits(
            "watch rebuilds when tsconfig include pattern adds file",
            file_map(&[
                ("/home/src/workspaces/project/index.ts", "const x = 1;"),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"compilerOptions": {},
	"include": ["*.ts"]
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "widen include pattern to add src dir",
                |sys| {
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/src/extra.ts",
                        "export const extra = 2;",
                    );
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/tsconfig.json",
                        r#"{
	"compilerOptions": {},
	"include": ["*.ts", "src/**/*.ts"]
}"#,
                    );
                },
            )],
        ),
        tsc_input_with_edits(
            "watch rebuilds when tsconfig is modified to change strict",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    "const x = null; const y: string = x;",
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("enable strict mode", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{"compilerOptions": {"strict": true}}"#,
                );
            })],
        ),
        tsc_input_with_edits(
            "watch detects file added to previously non-existent include path",
            file_map(&[
                ("/home/src/workspaces/project/index.ts", "const x = 1;"),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"compilerOptions": {},
	"include": ["index.ts", "src/**/*.ts"]
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "create src dir with ts file matching include",
                |sys| {
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/src/helper.ts",
                        r#"export const helper = "added";"#,
                    );
                },
            )],
        ),
        tsc_input_with_edits(
            "watch detects new file in existing include directory",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/a.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"compilerOptions": {},
	"include": ["src/**/*.ts"]
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "add new file to existing src directory",
                |sys| {
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/src/b.ts",
                        "export const b = 2;",
                    );
                },
            )],
        ),
        tsc_input_with_edits(
            "watch detects file added in new nested subdirectory",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/a.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"compilerOptions": {},
	"include": ["src/**/*.ts"]
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("create nested dir with ts file", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/src/deep/nested/util.ts",
                    r#"export const util = "nested";"#,
                );
            })],
        ),
        tsc_input_with_edits(
            "watch detects file added in multiple new subdirectories simultaneously",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/a.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"compilerOptions": {},
	"include": ["src/**/*.ts"]
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "create multiple new subdirs with files",
                |sys| {
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/src/models/user.ts",
                        "export interface User { name: string; }",
                    );
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/src/utils/format.ts",
                        "export function format(s: string): string { return s.trim(); }",
                    );
                },
            )],
        ),
        tsc_input_with_edits(
            "watch detects nested subdirectory removed and recreated",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/lib/helper.ts",
                    r#"export const helper = "v1";"#,
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"compilerOptions": {},
	"include": ["src/**/*.ts"]
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![
                tsc_edit_with_diff(
                    "remove nested dir",
                    "incremental has prior state and does not report no-inputs error",
                    |sys| sys.remove_no_error("/home/src/workspaces/project/src/lib/helper.ts"),
                ),
                new_tsc_edit("recreate nested dir with new content", |sys| {
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/src/lib/helper.ts",
                        r#"export const helper = "v2";"#,
                    );
                }),
            ],
        ),
        tsc_input_with_edits(
            "watch detects node modules package added",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { lib } from "mylib";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("install package in node_modules", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/mylib/package.json",
                    r#"{"name": "mylib", "main": "index.js", "types": "index.d.ts"}"#,
                );
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/mylib/index.js",
                    r#"exports.lib = "hello";"#,
                );
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/mylib/index.d.ts",
                    "export declare const lib: string;",
                );
            })],
        ),
        tsc_input_with_edits(
            "watch detects node modules package removed",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { lib } from "mylib";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
                (
                    "/home/src/workspaces/project/node_modules/mylib/package.json",
                    r#"{"name": "mylib", "main": "index.js", "types": "index.d.ts"}"#,
                ),
                (
                    "/home/src/workspaces/project/node_modules/mylib/index.js",
                    r#"exports.lib = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/node_modules/mylib/index.d.ts",
                    "export declare const lib: string;",
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("remove node_modules package", |sys| {
                sys.remove_no_error("/home/src/workspaces/project/node_modules/mylib/index.d.ts");
                sys.remove_no_error("/home/src/workspaces/project/node_modules/mylib/index.js");
                sys.remove_no_error("/home/src/workspaces/project/node_modules/mylib/package.json");
            })],
        ),
        tsc_input_with_edits(
            "watch handles tsconfig deleted",
            file_map(&[
                ("/home/src/workspaces/project/index.ts", "const x = 1;"),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![tsc_edit_with_diff(
                "delete tsconfig",
                "incremental reports config read error while clean build without tsconfig prints usage help",
                |sys| sys.remove_no_error("/home/src/workspaces/project/tsconfig.json"),
            )],
        ),
        tsc_input_with_edits(
            "watch handles tsconfig with extends base modified",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    "const x = null; const y: string = x;",
                ),
                (
                    "/home/src/workspaces/project/base.json",
                    r#"{
	"compilerOptions": { "strict": false }
}"#,
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"extends": "./base.json"
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("modify base config to enable strict", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/base.json",
                    r#"{
	"compilerOptions": { "strict": true }
}"#,
                );
            })],
        ),
        tsc_input_with_edits(
            "watch rebuilds when tsconfig is touched but content unchanged",
            file_map(&[
                ("/home/src/workspaces/project/index.ts", "const x = 1;"),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "touch tsconfig without changing content",
                |sys| {
                    let content =
                        sys.read_file_no_error("/home/src/workspaces/project/tsconfig.json");
                    sys.write_file_no_error("/home/src/workspaces/project/tsconfig.json", &content);
                },
            )],
        ),
        tsc_input_with_edits(
            "watch with tsconfig files list entry deleted",
            file_map(&[
                ("/home/src/workspaces/project/a.ts", "export const a = 1;"),
                ("/home/src/workspaces/project/b.ts", "export const b = 2;"),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    r#"{
	"compilerOptions": {},
	"files": ["a.ts", "b.ts"]
}"#,
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("delete file listed in files array", |sys| {
                sys.remove_no_error("/home/src/workspaces/project/b.ts");
            })],
        ),
        tsc_input_with_edits(
            "watch detects module going missing then coming back",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { util } from "./util";"#,
                ),
                (
                    "/home/src/workspaces/project/util.ts",
                    r#"export const util = "v1";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![
                tsc_edit_with_diff(
                    "delete util module",
                    "incremental resolves to .js output from prior build while clean build cannot find module",
                    |sys| sys.remove_no_error("/home/src/workspaces/project/util.ts"),
                ),
                new_tsc_edit("recreate util module with new content", |sys| {
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/util.ts",
                        r#"export const util = "v2";"#,
                    );
                }),
            ],
        ),
        tsc_input_with_edits(
            "watch detects scoped package installed",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { lib } from "@scope/mylib";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("install scoped package", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/@scope/mylib/package.json",
                    r#"{"name": "@scope/mylib", "types": "index.d.ts"}"#,
                );
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/@scope/mylib/index.d.ts",
                    "export declare const lib: string;",
                );
            })],
        ),
        tsc_input_with_edits(
            "watch detects package json types field edited",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { lib } from "mylib";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
                (
                    "/home/src/workspaces/project/node_modules/mylib/package.json",
                    r#"{"name": "mylib", "types": "old.d.ts"}"#,
                ),
                (
                    "/home/src/workspaces/project/node_modules/mylib/old.d.ts",
                    "export declare const lib: number;",
                ),
                (
                    "/home/src/workspaces/project/node_modules/mylib/new.d.ts",
                    "export declare const lib: string;",
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("change package.json types field", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/mylib/package.json",
                    r#"{"name": "mylib", "types": "new.d.ts"}"#,
                );
            })],
        ),
        tsc_input_with_edits(
            "watch detects at-types package installed later",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import * as lib from "untyped-lib";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
                (
                    "/home/src/workspaces/project/node_modules/untyped-lib/index.js",
                    "module.exports = {};",
                ),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit("install @types for the library", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/@types/untyped-lib/index.d.ts",
                    r#"declare module "untyped-lib" { export const value: string; }"#,
                );
                sys.write_file_no_error(
                    "/home/src/workspaces/project/node_modules/@types/untyped-lib/package.json",
                    r#"{"name": "@types/untyped-lib", "types": "index.d.ts"}"#,
                );
            })],
        ),
        tsc_input_with_edits(
            "watch detects file renamed and renamed back",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { helper } from "./helper";"#,
                ),
                (
                    "/home/src/workspaces/project/helper.ts",
                    "export const helper = 1;",
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![
                tsc_edit_with_diff(
                    "rename helper to helper2",
                    "incremental resolves to .js output from prior build while clean build cannot find module",
                    |sys| {
                        sys.rename_file_no_error(
                            "/home/src/workspaces/project/helper.ts",
                            "/home/src/workspaces/project/helper2.ts",
                        );
                    },
                ),
                new_tsc_edit("rename back to helper", |sys| {
                    sys.rename_file_no_error(
                        "/home/src/workspaces/project/helper2.ts",
                        "/home/src/workspaces/project/helper.ts",
                    );
                }),
            ],
        ),
        tsc_input_with_edits(
            "watch detects file deleted and new file added simultaneously",
            file_map(&[
                (
                    "/home/src/workspaces/project/a.ts",
                    r#"import { b } from "./b";"#,
                ),
                ("/home/src/workspaces/project/b.ts", "export const b = 1;"),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "delete b.ts and create c.ts with updated import",
                |sys| {
                    sys.remove_no_error("/home/src/workspaces/project/b.ts");
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/c.ts",
                        "export const c = 2;",
                    );
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/a.ts",
                        r#"import { c } from "./c";"#,
                    );
                },
            )],
        ),
        tsc_input_with_edits(
            "watch handles file rapidly recreated",
            file_map(&[
                (
                    "/home/src/workspaces/project/index.ts",
                    r#"import { val } from "./data";"#,
                ),
                (
                    "/home/src/workspaces/project/data.ts",
                    r#"export const val = "original";"#,
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
            ]),
            vec!["--watch"],
            vec![new_tsc_edit(
                "delete and immediately recreate with new content",
                |sys| {
                    sys.remove_no_error("/home/src/workspaces/project/data.ts");
                    sys.write_file_no_error(
                        "/home/src/workspaces/project/data.ts",
                        r#"export const val = "recreated";"#,
                    );
                },
            )],
        ),
        tsc_input_with_edits(
            "watch detects change in symlinked file",
            symlink_files,
            vec!["--watch"],
            vec![new_tsc_edit("modify symlink target", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/shared/index.ts",
                    r#"export const shared = "v2";"#,
                );
            })],
        ),
    ];

    for test in test_cases {
        test.run(&mut t, "commandLineWatch");
    }
}

fn list_to_tsconfig(base: &str, tsconfig_opts: &[&str]) -> (String, String) {
    let option_string = tsconfig_opts.join(",\n            ");
    let mut tsconfig_text = "{\n\t\"compilerOptions\": {\n".to_owned();
    let mut after = "            ";
    if !base.is_empty() {
        tsconfig_text.push_str("            ");
        tsconfig_text.push_str(base);
        after = ",\n            ";
    }
    if !tsconfig_opts.is_empty() {
        tsconfig_text.push_str(after);
        tsconfig_text.push_str(&option_string);
    }
    tsconfig_text.push_str("\n\t}\n}");
    (tsconfig_text, option_string)
}

fn to_tsconfig(base: &str, compiler_opts: &str) -> String {
    list_to_tsconfig(base, &[compiler_opts]).0
}

fn no_emit_watch_test_input(
    sub_scenario: &str,
    command_line_args: Vec<&str>,
    a_text: &str,
    tsconfig_options: &[&str],
    reintroduce_error: fn(&mut TestSys),
    emit_after_fix: fn(&mut TestSys),
    no_emit_after_fix: fn(&mut TestSys),
    emit_when_error: fn(&mut TestSys),
    no_emit_when_error: fn(&mut TestSys),
) -> TscInput {
    let no_emit_opt = r#""noEmit": true"#;
    let (tsconfig_text, _) = list_to_tsconfig(no_emit_opt, tsconfig_options);
    tsc_input_with_edits(
        sub_scenario,
        file_map(&[
            ("/home/src/workspaces/project/a.ts", a_text),
            ("/home/src/workspaces/project/tsconfig.json", &tsconfig_text),
        ]),
        command_line_args,
        vec![
            new_tsc_edit("fix error", |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/a.ts",
                    r#"const a = "hello";"#,
                );
            }),
            new_tsc_edit("emit after fixing error", emit_after_fix),
            new_tsc_edit("no emit run after fixing error", no_emit_after_fix),
            new_tsc_edit("introduce error", reintroduce_error),
            new_tsc_edit("emit when error", emit_when_error),
            new_tsc_edit("no emit run when error", no_emit_when_error),
        ],
    )
}

#[test]
fn test_tsc_no_emit_watch() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        no_emit_watch_test_input(
            "syntax errors",
            vec!["-w"],
            r#"const a = "hello"#,
            &[],
            |sys| {
                sys.write_file_no_error("/home/src/workspaces/project/a.ts", r#"const a = "hello"#)
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, ""),
                )
            },
        ),
        no_emit_watch_test_input(
            "semantic errors",
            vec!["-w"],
            r#"const a: number = "hello""#,
            &[],
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/a.ts",
                    r#"const a: number = "hello""#,
                );
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, ""),
                )
            },
        ),
        no_emit_watch_test_input(
            "dts errors without dts enabled",
            vec!["-w"],
            "const a = class { private p = 10; };",
            &[],
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/a.ts",
                    "const a = class { private p = 10; };",
                );
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", ""),
                )
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, ""),
                )
            },
        ),
        no_emit_watch_test_input(
            "dts errors",
            vec!["-w"],
            "const a = class { private p = 10; };",
            &[r#""declaration": true"#],
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/a.ts",
                    "const a = class { private p = 10; };",
                );
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", r#""declaration": true"#),
                );
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, r#""declaration": true"#),
                );
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig("", r#""declaration": true"#),
                );
            },
            |sys| {
                sys.write_file_no_error(
                    "/home/src/workspaces/project/tsconfig.json",
                    &to_tsconfig(r#""noEmit": true"#, r#""declaration": true"#),
                );
            },
        ),
    ];

    for test in test_cases {
        test.run(&mut t, "noEmit");
    }
}
