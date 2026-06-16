use std::collections::HashMap;
use std::time::SystemTime;

use ts_testutil::stringtestutil;
use ts_vfs::vfstest::IntoMapFile;

use super::FileMap;
use super::runner::{TestingT, TscEdit, TscInput};
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

fn tsc_input(sub_scenario: &str, command_line_args: Vec<&str>) -> TscInput {
    tsc_input_with_files(sub_scenario, FileMap::new(), command_line_args)
}

fn tsc_input_with_env(
    sub_scenario: &str,
    command_line_args: Vec<&str>,
    env: &[(&str, &str)],
) -> TscInput {
    let mut input = tsc_input(sub_scenario, command_line_args);
    input.env = env
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    input
}

fn tsc_input_with_files(
    sub_scenario: &str,
    files: FileMap,
    command_line_args: Vec<&str>,
) -> TscInput {
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

fn tsc_input_with_files_edits(sub_scenario: &str, files: FileMap, edits: Vec<TscEdit>) -> TscInput {
    let mut input = tsc_input_with_files(sub_scenario, files, vec![]);
    input.edits = edits;
    input
}

fn tsc_input_with_files_cwd(sub_scenario: &str, files: FileMap, cwd: &str) -> TscInput {
    let mut input = tsc_input_with_files(sub_scenario, files, vec![]);
    input.cwd = cwd.to_owned();
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

fn replace_module_none_with_es2015(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/tsconfig.json",
        "none",
        "es2015",
    );
}

fn strict_no_emit_config() -> String {
    stringtestutil::dedent(
        r#"
        {
            "compilerOptions": {
                "strict": true,
                "noEmit": true
            }
        }"#,
    )
}

#[test]
fn test_tsc_commandline() {
    let mut t = RustTestingT;
    t.parallel();

    let strict_project = || {
        file_map(&[
            (
                "/home/src/workspaces/project/first.ts",
                "export const a = 1",
            ),
            (
                "/home/src/workspaces/project/tsconfig.json",
                &strict_no_emit_config(),
            ),
        ])
    };

    let test_cases = vec![
        tsc_input_with_env(
            "show help with ExitStatus.DiagnosticsPresent_OutputsSkipped",
            vec![],
            &[("TS_TEST_TERMINAL_WIDTH", "120")],
        ),
        tsc_input(
            "show help with ExitStatus.DiagnosticsPresent_OutputsSkipped when host cannot provide terminal width",
            vec![],
        ),
        tsc_input_with_env(
            "does not add color when NO_COLOR is set",
            vec![],
            &[("NO_COLOR", "true")],
        ),
        tsc_input_with_env(
            "adds color when FORCE_COLOR is set",
            vec![],
            &[("FORCE_COLOR", "true")],
        ),
        tsc_input_with_env(
            "does not add color when NO_COLOR is set even if FORCE_COLOR is set",
            vec![],
            &[("NO_COLOR", "true"), ("FORCE_COLOR", "true")],
        ),
        tsc_input(
            "when build not first argument",
            vec!["--verbose", "--build"],
        ),
        tsc_input(
            "Initialized TSConfig with files options",
            vec!["--init", "file0.st", "file1.ts", "file2.ts"],
        ),
        tsc_input(
            "Initialized TSConfig with boolean value compiler options",
            vec!["--init", "--noUnusedLocals"],
        ),
        tsc_input(
            "Initialized TSConfig with enum value compiler options",
            vec!["--init", "--target", "es5", "--jsx", "react"],
        ),
        tsc_input(
            "Initialized TSConfig with list compiler options",
            vec!["--init", "--types", "jquery,mocha"],
        ),
        tsc_input(
            "Initialized TSConfig with list compiler options with enum value",
            vec!["--init", "--lib", "es5,es2015.core"],
        ),
        tsc_input(
            "Initialized TSConfig with incorrect compiler option",
            vec!["--init", "--someNonExistOption"],
        ),
        tsc_input(
            "Initialized TSConfig with incorrect compiler option value",
            vec!["--init", "--lib", "nonExistLib,es5,es2015.promise"],
        ),
        tsc_input(
            "Initialized TSConfig with advanced options",
            vec![
                "--init",
                "--declaration",
                "--declarationDir",
                "lib",
                "--skipLibCheck",
                "--noErrorTruncation",
            ],
        ),
        tsc_input("Initialized TSConfig with --help", vec!["--init", "--help"]),
        tsc_input(
            "Initialized TSConfig with --watch",
            vec!["--init", "--watch"],
        ),
        tsc_input_with_files(
            "Initialized TSConfig with tsconfig.json",
            strict_project(),
            vec!["--init"],
        ),
        tsc_input("help", vec!["--help"]),
        tsc_input("help all", vec!["--help", "--all"]),
        tsc_input_with_files(
            "Parse --lib option with file name",
            file_map(&[(
                "/home/src/workspaces/project/first.ts",
                "export const Key = Symbol()",
            )]),
            vec!["--lib", "es6 ", "first.ts"],
        ),
        tsc_input_with_files("Project is empty string", strict_project(), vec![]),
        tsc_input_with_files("Parse -p", strict_project(), vec!["-p", "."]),
        tsc_input_with_files(
            "Parse -p with path to tsconfig file",
            strict_project(),
            vec!["-p", "/home/src/workspaces/project/tsconfig.json"],
        ),
        tsc_input_with_files(
            "Parse -p with path to tsconfig folder",
            strict_project(),
            vec!["-p", "/home/src/workspaces/project"],
        ),
        tsc_input(
            "Parse enum type options",
            vec![
                "--moduleResolution",
                "nodenext ",
                "first.ts",
                "--module",
                "nodenext",
                "--target",
                "esnext",
                "--moduleDetection",
                "auto",
                "--jsx",
                "react",
                "--newLine",
                "crlf",
            ],
        ),
        tsc_input_with_files(
            "Parse watch interval option",
            strict_project(),
            vec!["-w", "--watchInterval", "1000"],
        ),
        tsc_input(
            "Parse watch interval option without tsconfig.json",
            vec!["-w", "--watchInterval", "1000"],
        ),
        tsc_input_with_files(
            "Config with references and empty file and refers to config with noEmit",
            file_map(&[
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"{
                    "files": [],
                    "references": [
                        {
                            "path": "./packages/pkg1"
                        },
                    ],
                }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/packages/pkg1/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"{
                    "compilerOptions": {
                        "composite": true,
                        "noEmit": true
                    },
                    "files": [
                        "./index.ts",
                    ],
                }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/packages/pkg1/index.ts",
                    "export const a = 1;",
                ),
            ]),
            vec!["-p", "."],
        ),
        tsc_input("locale", vec!["--locale", "cs", "--version"]),
        tsc_input("bad locale", vec!["--locale", "whoops", "--version"]),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "commandLine");
    }
}

#[test]
fn test_tsc_composite() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_files(
            "when setting composite false on command line",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/main.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "target": "es5",
                                "module": "commonjs",
                                "composite": true,
                            },
                            "include": [
                                "src/**/*.ts",
                            ],
                        }"#,
                    ),
                ),
            ]),
            vec!["--composite", "false"],
        ),
        tsc_input_with_files(
            "when setting composite null on command line",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/main.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "target": "es5",
                                "module": "commonjs",
                                "composite": true,
                            },
                            "include": [
                                "src/**/*.ts",
                            ],
                        }"#,
                    ),
                ),
            ]),
            vec!["--composite", "null"],
        ),
        tsc_input_with_files(
            "when setting composite false on command line but has tsbuild info in config",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/main.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "target": "es5",
                                "module": "commonjs",
                                "composite": true,
                                "tsBuildInfoFile": "tsconfig.json.tsbuildinfo",
                            },
                            "include": [
                                "src/**/*.ts",
                            ],
                        }"#,
                    ),
                ),
            ]),
            vec!["--composite", "false"],
        ),
        tsc_input_with_files(
            "when setting composite false and tsbuildinfo as null on command line but has tsbuild info in config",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/main.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "target": "es5",
                                "module": "commonjs",
                                "composite": true,
                                "tsBuildInfoFile": "tsconfig.json.tsbuildinfo",
                            },
                            "include": [
                                "src/**/*.ts",
                            ],
                        }"#,
                    ),
                ),
            ]),
            vec!["--composite", "false", "--tsBuildInfoFile", "null"],
        ),
        tsc_input_with_files_edits(
            "converting to modules",
            file_map(&[
                ("/home/src/workspaces/project/src/main.ts", "const x = 10;"),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "module": "none",
                                "composite": true,
                            },
                        }"#,
                    ),
                ),
            ]),
            vec![TscEdit {
                caption: "convert to modules".to_owned(),
                command_line_args: None,
                edit: Some(replace_module_none_with_es2015),
                expected_diff: String::new(),
            }],
        ),
        tsc_input_with_files_cwd(
            "synthetic jsx import of ESM module from CJS module no crash no jsx element",
            file_map(&[
                (
                    "/home/src/projects/project/src/main.ts",
                    "export default 42;",
                ),
                (
                    "/home/src/projects/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "composite": true,
                                "module": "Node16",
                                "jsx": "react-jsx",
                                "jsxImportSource": "solid-js",
                            },
                        }"#,
                    ),
                ),
                (
                    "/home/src/projects/project/node_modules/solid-js/package.json",
                    &stringtestutil::dedent(
                        r#"
                            {
                                "name": "solid-js",
                                "type": "module"
                            }
                        "#,
                    ),
                ),
                (
                    "/home/src/projects/project/node_modules/solid-js/jsx-runtime.d.ts",
                    &stringtestutil::dedent(
                        r#"
                            export namespace JSX {
                                type IntrinsicElements = { div: {}; };
                            }
                        "#,
                    ),
                ),
            ]),
            "/home/src/projects/project",
        ),
        tsc_input_with_files_cwd(
            "synthetic jsx import of ESM module from CJS module error on jsx element",
            file_map(&[
                (
                    "/home/src/projects/project/src/main.tsx",
                    "export default <div/>;",
                ),
                (
                    "/home/src/projects/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "composite": true,
                                "module": "Node16",
                                "jsx": "react-jsx",
                                "jsxImportSource": "solid-js",
                            },
                        }"#,
                    ),
                ),
                (
                    "/home/src/projects/project/node_modules/solid-js/package.json",
                    &stringtestutil::dedent(
                        r#"
                            {
                                "name": "solid-js",
                                "type": "module"
                            }
                        "#,
                    ),
                ),
                (
                    "/home/src/projects/project/node_modules/solid-js/jsx-runtime.d.ts",
                    &stringtestutil::dedent(
                        r#"
                            export namespace JSX {
                                type IntrinsicElements = { div: {}; };
                            }
                        "#,
                    ),
                ),
            ]),
            "/home/src/projects/project",
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "composite");
    }
}
