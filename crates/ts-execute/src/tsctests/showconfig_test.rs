use std::collections::HashMap;
use std::time::SystemTime;

use ts_testutil::stringtestutil;
use ts_vfs::vfstest::IntoMapFile;

use super::FileMap;
use super::runner::{TestingT, TscInput};

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

fn tsc_input_with_files(
    sub_scenario: &str,
    files: FileMap,
    command_line_args: Vec<&str>,
) -> TscInput {
    TscInput {
        sub_scenario: sub_scenario.to_owned(),
        command_line_args: command_line_args.into_iter().map(str::to_owned).collect(),
        files,
        cwd: "/home/src/workspaces/project".to_owned(),
        edits: Vec::new(),
        env: HashMap::new(),
        ignore_case: false,
        windows_style_root: String::new(),
    }
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

#[test]
fn test_show_config() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input("Default initialized TSConfig", vec!["--showConfig"]),
        tsc_input(
            "Show TSConfig with files options",
            vec!["--showConfig", "file0.ts", "file1.ts", "file2.ts"],
        ),
        tsc_input(
            "Show TSConfig with boolean value compiler options",
            vec!["--showConfig", "--noUnusedLocals"],
        ),
        tsc_input(
            "Show TSConfig with enum value compiler options",
            vec!["--showConfig", "--target", "es5", "--jsx", "react"],
        ),
        tsc_input(
            "Show TSConfig with list compiler options",
            vec!["--showConfig", "--types", "jquery,mocha"],
        ),
        tsc_input(
            "Show TSConfig with list compiler options with enum value",
            vec!["--showConfig", "--lib", "es5,es2015.core"],
        ),
        tsc_input(
            "Show TSConfig with incorrect compiler option",
            vec!["--showConfig", "--someNonExistOption"],
        ),
        tsc_input(
            "Show TSConfig with incorrect compiler option value",
            vec!["--showConfig", "--lib", "nonExistLib,es5,es2015.promise"],
        ),
        tsc_input(
            "Show TSConfig with advanced options",
            vec![
                "--showConfig",
                "--declaration",
                "--declarationDir",
                "lib",
                "--skipLibCheck",
                "--noErrorTruncation",
            ],
        ),
        tsc_input_with_files(
            "Show TSConfig with compileOnSave and more",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "esModuleInterop": true,
                                "target": "es5",
                                "module": "commonjs",
                                "strict": true
                            },
                            "compileOnSave": true,
                            "exclude": [
                                "dist"
                            ],
                            "files": [],
                            "include": [
                                "src/*"
                            ],
                            "references": [
                                { "path": "./test" }
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
        tsc_input_with_files(
            "Show TSConfig with paths and more",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "allowJs": true,
                                "outDir": "./lib",
                                "esModuleInterop": true,
                                "module": "commonjs",
                                "moduleResolution": "node",
                                "target": "ES2017",
                                "sourceMap": true,
                                "baseUrl": ".",
                                "paths": {
                                    "@root/*": ["./*"],
                                    "@configs/*": ["src/configs/*"],
                                    "@common/*": ["src/common/*"],
                                    "*": [
                                        "node_modules/*",
                                        "src/types/*"
                                    ]
                                },
                                "experimentalDecorators": true,
                                "emitDecoratorMetadata": true,
                                "resolveJsonModule": true
                            },
                            "include": [
                                "./src/**/*"
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
        tsc_input_with_files(
            "Show TSConfig with include filtering files",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/main.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/src/util.ts",
                    "export const b = 2;",
                ),
                (
                    "/home/src/workspaces/project/extra.ts",
                    "export const c = 3;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "strict": true
                            },
                            "include": [
                                "src/**/*"
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
        tsc_input_with_files(
            "Show TSConfig with references",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "composite": true,
                                "strict": true
                            },
                            "references": [
                                { "path": "./packages/a" },
                                { "path": "./packages/b" }
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
        tsc_input_with_files(
            "Show TSConfig with exclude",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/test/test1.ts",
                    "import { a } from \"../src\";",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "strict": true
                            },
                            "exclude": [
                                "test"
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
        tsc_input_with_files(
            "Show TSConfig with files and include",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/main.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/extra.ts",
                    "export const c = 3;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "strict": true
                            },
                            "files": [
                                "extra.ts"
                            ],
                            "include": [
                                "src/**/*"
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
        tsc_input_with_files(
            "Show TSConfig with transitively implied options",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "module": "nodenext"
                            }
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
        tsc_input_with_files(
            "Show TSConfig with exclude and outDir",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const a = 1;",
                ),
                (
                    "/home/src/workspaces/project/src/bin/tool.ts",
                    "export const b = 2;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "strict": true,
                                "outDir": "./build"
                            },
                            "exclude": [
                                "build"
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["-p", "tsconfig.json", "--showConfig"],
        ),
    ];

    for test in test_cases {
        test.run(&mut t, "showConfig");
    }
}
