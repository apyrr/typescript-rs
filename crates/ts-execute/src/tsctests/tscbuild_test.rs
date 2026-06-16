use std::collections::HashMap;
use std::time::SystemTime;

use ts_core as core;
use ts_testutil::{harnessutil, stringtestutil};
use ts_tsoptions as tsoptions;
use ts_vfs::Fs as _;
use ts_vfs::vfstest::{self, IntoMapFile};

use super::FileMap;
use super::runner::{TestingT, TscEdit, TscInput, no_change, no_change_only_edit};
use super::sys::{
    TSC_LIB_PATH, TestSys, get_test_lib_path_for, new_test_sys, tsc_default_lib_content,
};

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

fn tsc_input_with_cwd(
    sub_scenario: &str,
    files: FileMap,
    cwd: &str,
    command_line_args: Vec<&str>,
) -> TscInput {
    let mut input = tsc_input(sub_scenario, files, command_line_args);
    input.cwd = cwd.to_owned();
    input
}

fn tsc_input_with_cwd_edits(
    sub_scenario: &str,
    files: FileMap,
    cwd: &str,
    command_line_args: Vec<&str>,
    edits: Vec<TscEdit>,
) -> TscInput {
    let mut input = tsc_input_with_cwd(sub_scenario, files, cwd, command_line_args);
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

fn edit_command(caption: &str, command_line_args: Vec<&str>) -> TscEdit {
    TscEdit {
        caption: caption.to_owned(),
        command_line_args: Some(command_line_args.into_iter().map(str::to_owned).collect()),
        edit: None,
        expected_diff: String::new(),
    }
}

fn edit_command_with_change(
    caption: &str,
    command_line_args: Vec<&str>,
    edit: fn(&mut TestSys),
) -> TscEdit {
    TscEdit {
        caption: caption.to_owned(),
        command_line_args: Some(command_line_args.into_iter().map(str::to_owned).collect()),
        edit: Some(edit),
        expected_diff: String::new(),
    }
}

fn build_command_line_different_options_map(option_name: &str) -> FileMap {
    let mut files = FileMap::new();
    files.insert(
        "/home/src/workspaces/project/tsconfig.json".to_owned(),
        stringtestutil::dedent(&format!(
            r#"
            {{
                "compilerOptions": {{
                    "{option_name}": true
                }}
            }}"#
        ))
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
    for (path, text) in [
        (
            "/home/src/workspaces/project/a.ts",
            "export const a = 10;const aLocal = 10;",
        ),
        (
            "/home/src/workspaces/project/b.ts",
            "export const b = 10;const bLocal = 10;",
        ),
        (
            "/home/src/workspaces/project/c.ts",
            r#"import { a } from "./a";export const c = a;"#,
        ),
        (
            "/home/src/workspaces/project/d.ts",
            r#"import { b } from "./b";export const d = b;"#,
        ),
    ] {
        files.insert(path.to_owned(), text.into_map_file(SystemTime::UNIX_EPOCH));
    }
    files
}

fn build_command_line_emit_declaration_only_map(options: &[&str]) -> FileMap {
    let compiler_options_str = options
        .iter()
        .map(|opt| format!(r#""{opt}": true"#))
        .collect::<Vec<_>>()
        .join(", ");
    let mut files = FileMap::new();
    for (path, text) in [
        (
            "/home/src/workspaces/solution/project1/src/tsconfig.json",
            stringtestutil::dedent(&format!(
                r#"
                {{
                    "compilerOptions": {{ {compiler_options_str} }}
                }}"#
            )),
        ),
        (
            "/home/src/workspaces/solution/project1/src/a.ts",
            "export const a = 10;const aLocal = 10;".to_owned(),
        ),
        (
            "/home/src/workspaces/solution/project1/src/b.ts",
            "export const b = 10;const bLocal = 10;".to_owned(),
        ),
        (
            "/home/src/workspaces/solution/project1/src/c.ts",
            r#"import { a } from "./a";export const c = a;"#.to_owned(),
        ),
        (
            "/home/src/workspaces/solution/project1/src/d.ts",
            r#"import { b } from "./b";export const d = b;"#.to_owned(),
        ),
        (
            "/home/src/workspaces/solution/project2/src/tsconfig.json",
            stringtestutil::dedent(&format!(
                r#"
                {{
                    "compilerOptions": {{ {compiler_options_str} }},
                    "references": [{{ "path": "../../project1/src" }}]
                }}"#
            )),
        ),
        (
            "/home/src/workspaces/solution/project2/src/e.ts",
            "export const e = 10;".to_owned(),
        ),
        (
            "/home/src/workspaces/solution/project2/src/f.ts",
            r#"import { a } from "../../project1/src/a"; export const f = a;"#.to_owned(),
        ),
        (
            "/home/src/workspaces/solution/project2/src/g.ts",
            r#"import { b } from "../../project1/src/b"; export const g = b;"#.to_owned(),
        ),
    ] {
        files.insert(path.to_owned(), text.into_map_file(SystemTime::UNIX_EPOCH));
    }
    files
}

fn replace_a_local(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/a.ts",
        "Local = 1",
        "Local = 10",
    );
}

fn append_solution_project1_a_local(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/project1/src/a.ts",
        "const aa = 10;",
    );
}

fn append_solution_project1_a_export(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/project1/src/a.ts",
        "export const aaa = 10;",
    );
}

fn append_solution_project1_b_local_alocal(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/project1/src/b.ts",
        "const alocal = 10;",
    );
}

fn append_solution_project1_b_local_aaaa(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/project1/src/b.ts",
        "const aaaa = 10;",
    );
}

fn append_solution_project1_b_export_aaaaa(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/project1/src/b.ts",
        "export const aaaaa = 10;",
    );
}

fn append_solution_project1_b_export_a2(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/project1/src/b.ts",
        "export const a2 = 10;",
    );
}

fn append_solution_project1_b_local_blocal(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/project1/src/b.ts",
        "const blocal = 10;",
    );
}

fn local_change(caption: &str, edit: fn(&mut TestSys)) -> TscEdit {
    TscEdit {
        caption: caption.to_owned(),
        command_line_args: None,
        edit: Some(edit),
        expected_diff: String::new(),
    }
}

fn local_change_expected(caption: &str, edit: fn(&mut TestSys), expected_diff: &str) -> TscEdit {
    TscEdit {
        caption: caption.to_owned(),
        command_line_args: None,
        edit: Some(edit),
        expected_diff: expected_diff.to_owned(),
    }
}

fn build_command_line_emit_declaration_only_test_cases(
    options: &[&str],
    suffix: &str,
) -> Vec<TscInput> {
    let mut options_with_emit_declaration_only = options.to_vec();
    options_with_emit_declaration_only.push("emitDeclarationOnly");

    vec![
        tsc_input_with_cwd_edits(
            &format!("emitDeclarationOnly on commandline{suffix}"),
            build_command_line_emit_declaration_only_map(options),
            "/home/src/workspaces/solution",
            vec!["--b", "project2/src", "--verbose", "--emitDeclarationOnly"],
            vec![
                no_change(),
                local_change("local change", append_solution_project1_a_local),
                local_change("non local change", append_solution_project1_a_export),
                edit_command("emit js files", vec!["--b", "project2/src", "--verbose"]),
                no_change(),
                edit_command_with_change(
                    "js emit with change without emitDeclarationOnly",
                    vec!["--b", "project2/src", "--verbose"],
                    append_solution_project1_b_local_alocal,
                ),
                local_change("local change", append_solution_project1_b_local_aaaa),
                local_change("non local change", append_solution_project1_b_export_aaaaa),
                edit_command_with_change(
                    "js emit with change without emitDeclarationOnly",
                    vec!["--b", "project2/src", "--verbose"],
                    append_solution_project1_b_export_a2,
                ),
            ],
        ),
        tsc_input_with_cwd_edits(
            &format!("emitDeclarationOnly false on commandline{suffix}"),
            build_command_line_emit_declaration_only_map(&options_with_emit_declaration_only),
            "/home/src/workspaces/solution",
            vec!["--b", "project2/src", "--verbose"],
            vec![
                no_change(),
                local_change("change", append_solution_project1_a_local),
                edit_command(
                    "emit js files",
                    vec![
                        "--b",
                        "project2/src",
                        "--verbose",
                        "--emitDeclarationOnly",
                        "false",
                    ],
                ),
                no_change(),
                edit_command(
                    "no change run with js emit",
                    vec![
                        "--b",
                        "project2/src",
                        "--verbose",
                        "--emitDeclarationOnly",
                        "false",
                    ],
                ),
                edit_command_with_change(
                    "js emit with change",
                    vec![
                        "--b",
                        "project2/src",
                        "--verbose",
                        "--emitDeclarationOnly",
                        "false",
                    ],
                    append_solution_project1_b_local_blocal,
                ),
            ],
        ),
    ]
}

fn replace_config_comma_with_declaration(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/tsconfig.json",
        ",",
        r#", "declaration": true"#,
    );
}

fn append_foo_bar_to_a(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/project/a.ts",
        "export function fooBar() { }",
    );
}

fn touch_config_no_text_change(sys: &mut TestSys) {
    sys.replace_file_text("/home/src/workspaces/project/tsconfig.json", "", "");
}

fn write_fixed_config(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/home/src/workspaces/project/tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "composite": true, "declaration": true
                },
                "files": [
                    "a.ts",
                    "b.ts"
                ]
            }"#,
        ),
    );
}

fn demo_core_utilities() -> String {
    stringtestutil::dedent(
        r#"

        export function makeRandomName() {
            return "Bob!?! ";
        }

        export function lastElementOf<T>(arr: T[]): T | undefined {
            if (arr.length === 0) return undefined;
            return arr[arr.length - 1];
        }"#,
    )
}

fn demo_core_config_with_zoo_ref() -> String {
    stringtestutil::dedent(
        r#"
        {
            "extends": "../tsconfig-base.json",
            "compilerOptions": {
                "outDir": "../lib/core",
                "rootDir": "."
            },
            "references": [
                {
                    "path": "../zoo",
                }
            ]
        }"#,
    )
}

fn build_demo_file_map(modify: Option<fn(&mut FileMap)>) -> FileMap {
    let mut files = file_map(&[
        (
            "/user/username/projects/demo/animals/animal.ts",
            &stringtestutil::dedent(
                r#"
                export type Size = "small" | "medium" | "large";
                export default interface Animal {
                    size: Size;
                }
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/animals/dog.ts",
            &stringtestutil::dedent(
                r#"
                import Animal from '.';
                import { makeRandomName } from '../core/utilities';

                export interface Dog extends Animal {
                    woof(): void;
                    name: string;
                }

                export function createDog(): Dog {
                    return ({
                        size: "medium",
                        woof: function(this: Dog) {
                            console.log(`${ this.name } says "Woof"!`);
                        },
                        name: makeRandomName()
                    });
                }
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/animals/index.ts",
            &stringtestutil::dedent(
                r#"
                import Animal from './animal';

                export default Animal;
                import { createDog, Dog } from './dog';
                export { createDog, Dog };
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/animals/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig-base.json",
                    "compilerOptions": {
                        "outDir": "../lib/animals",
                        "rootDir": "."
                    },
                    "references": [
                        { "path": "../core" }
                    ]
                }
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/core/utilities.ts",
            &demo_core_utilities(),
        ),
        (
            "/user/username/projects/demo/core/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig-base.json",
                    "compilerOptions": {
                        "outDir": "../lib/core",
                        "rootDir": "."
                    },
                }
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/zoo/zoo.ts",
            &stringtestutil::dedent(
                r#"
                import { Dog, createDog } from '../animals/index';

                export function createZoo(): Array<Dog> {
                    return [
                        createDog()
                    ];
                }
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/zoo/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig-base.json",
                    "compilerOptions": {
                        "outDir": "../lib/zoo",
                        "rootDir": "."
                    },
                    "references": [
                        {
                            "path": "../animals"
                        }
                    ]
                }
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/tsconfig-base.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "declaration": true,
                        "target": "es5",
                        "module": "commonjs",
                        "strict": true,
                        "noUnusedLocals": true,
                        "noUnusedParameters": true,
                        "noImplicitReturns": true,
                        "noFallthroughCasesInSwitch": true,
                        "composite": true,
                    },
                }
            "#,
            ),
        ),
        (
            "/user/username/projects/demo/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "files": [],
                    "references": [
                        {
                            "path": "./core"
                        },
                        {
                            "path": "./animals",
                        },
                        {
                            "path": "./zoo",
                        },
                    ],
                }
            "#,
            ),
        ),
    ]);
    if let Some(modify) = modify {
        modify(&mut files);
    }
    files
}

fn demo_with_core_ref_to_zoo(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/demo/core/tsconfig.json".to_owned(),
        demo_core_config_with_zoo_ref().into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn demo_with_bad_ref(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/demo/core/utilities.ts".to_owned(),
        format!("import * as A from '../animals'\n{}", demo_core_utilities())
            .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn demo_with_circular_reference_option(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/demo/a/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "extends": "../tsconfig-base.json",
                "compilerOptions": {
                    "outDir": "../lib/a",
                    "rootDir": "."
                },
                "references": [
                    {
                        "path": "../b",
                        "circular": true
                    }
                ]
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.insert(
        "/user/username/projects/demo/b/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "extends": "../tsconfig-base.json",
                "compilerOptions": {
                    "outDir": "../lib/b",
                    "rootDir": "."
                },
                "references": [
                    {
                        "path": "../a",
                    }
                ]
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.insert(
        "/user/username/projects/demo/a/index.ts".to_owned(),
        "export const a = 10;".into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.insert(
        "/user/username/projects/demo/b/index.ts".to_owned(),
        "export const b = 10;".into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.insert(
        "/user/username/projects/demo/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "files": [],
                "references": [
                    {
                        "path": "./core"
                    },
                    {
                        "path": "./animals",
                    },
                    {
                        "path": "./zoo",
                    },
                    {
                        "path": "./a",
                    },
                    {
                        "path": "./b",
                    },
                ],
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn fix_demo_core_config(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/demo/core/tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "extends": "../tsconfig-base.json",
                "compilerOptions": {
                    "outDir": "../lib/core",
                    "rootDir": "."
                },
            }"#,
        ),
    );
}

fn prepend_blank_to_demo_core_utilities(sys: &mut TestSys) {
    sys.prepend_file("/user/username/projects/demo/core/utilities.ts", "\n");
}

fn build_emit_declaration_only_import_file_map(
    declaration_map: bool,
    circular_ref: bool,
) -> FileMap {
    let mut files = FileMap::new();
    for (path, text) in [
        (
            "/home/src/workspaces/project/src/a.ts",
            stringtestutil::dedent(
                r#"
                import { B } from "./b";

                export interface A {
                    b: B;
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/project/src/b.ts",
            stringtestutil::dedent(
                r#"
                import { C } from "./c";

                export interface B {
                    b: C;
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/project/src/c.ts",
            stringtestutil::dedent(
                r#"
                import { A } from "./a";

                export interface C {
                    a: A;
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/project/src/index.ts",
            stringtestutil::dedent(
                r#"
                export { A } from "./a";
                export { B } from "./b";
                export { C } from "./c";"#,
            ),
        ),
        (
            "/home/src/workspaces/project/tsconfig.json",
            stringtestutil::dedent(&format!(
                r#"
                {{
                    "compilerOptions": {{
                        "incremental": true,
                        "target": "es5",
                        "module": "commonjs",
                        "declaration": true,
                        "declarationMap": {declaration_map},
                        "sourceMap": true,
                        "outDir": "./lib",
                        "composite": true,
                        "strict": true,
                        "esModuleInterop": true,
                        "alwaysStrict": true,
                        "rootDir": "src",
                        "emitDeclarationOnly": true,
                    }},
                }}"#
            )),
        ),
    ] {
        files.insert(path.to_owned(), text.into_map_file(SystemTime::UNIX_EPOCH));
    }

    if !circular_ref {
        files.remove("/home/src/workspaces/project/src/index.ts");
        files.insert(
            "/home/src/workspaces/project/src/a.ts".to_owned(),
            stringtestutil::dedent(
                r#"
                export class B { prop = "hello"; }

                export interface A {
                    b: B;
                }"#,
            )
            .into_map_file(SystemTime::UNIX_EPOCH),
        );
    }
    files
}

fn emit_declaration_only_test_case(declaration_map: bool) -> TscInput {
    tsc_input_with_edits(
        &format!(
            "only dts output in circular import project with emitDeclarationOnly{}",
            if declaration_map {
                " and declarationMap"
            } else {
                ""
            }
        ),
        build_emit_declaration_only_import_file_map(declaration_map, true),
        vec!["--b", "--verbose"],
        vec![local_change(
            "incremental-declaration-changes",
            add_foo_to_emit_declaration_only_a,
        )],
    )
}

fn add_foo_to_emit_declaration_only_a(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/src/a.ts",
        "b: B;",
        "b: B; foo: any;",
    );
}

fn add_class_c_to_emit_declaration_only_a(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/src/a.ts",
        "export interface A {",
        &stringtestutil::dedent(
            r#"
            class C { }
            export interface A {"#,
        ),
    );
}

fn remove_child2_composite_outputs(sys: &mut TestSys) {
    sys.remove_no_error("/home/src/workspaces/solution/child/child2.ts");
    sys.remove_no_error("/home/src/workspaces/solution/child/child2.js");
    sys.remove_no_error("/home/src/workspaces/solution/child/child2.d.ts");
}

fn remove_child2_non_composite_outputs(sys: &mut TestSys) {
    sys.remove_no_error("/home/src/workspaces/solution/child/child2.ts");
    sys.remove_no_error("/home/src/workspaces/solution/child/child2.js");
}

fn build_inferred_type_from_transitive_module_map(
    isolated_modules: bool,
    lazy_extra_contents: &str,
) -> FileMap {
    let mut files = FileMap::new();
    for (path, text) in [
        (
            "/home/src/workspaces/project/bar.ts",
            stringtestutil::dedent(
                r#"
                interface RawAction {
                    (...args: any[]): Promise<any> | void;
                }
                interface ActionFactory {
                    <T extends RawAction>(target: T): T;
                }
                declare function foo<U extends any[] = any[]>(): ActionFactory;
                export default foo()(function foobar(param: string): void {
                });"#,
            ),
        ),
        (
            "/home/src/workspaces/project/bundling.ts",
            stringtestutil::dedent(
                r#"
                export class LazyModule<TModule> {
                    constructor(private importCallback: () => Promise<TModule>) {}
                }

                export class LazyAction<
                    TAction extends (...args: any[]) => any,
                    TModule
                >  {
                    constructor(_lazyModule: LazyModule<TModule>, _getter: (module: TModule) => TAction) {
                    }
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/project/global.d.ts",
            stringtestutil::dedent(
                r#"
                interface PromiseConstructor {
                    new <T>(): Promise<T>;
                }
                declare var Promise: PromiseConstructor;
                interface Promise<T> {
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/project/index.ts",
            stringtestutil::dedent(
                r#"
                import { LazyAction, LazyModule } from './bundling';
                const lazyModule = new LazyModule(() =>
                    import('./lazyIndex')
                );
                export const lazyBar = new LazyAction(lazyModule, m => m.bar);"#,
            ),
        ),
        (
            "/home/src/workspaces/project/lazyIndex.ts",
            format!(
                "{}{}",
                stringtestutil::dedent(
                    r#"
                    export { default as bar } from './bar';"#,
                ),
                lazy_extra_contents
            ),
        ),
        (
            "/home/src/workspaces/project/tsconfig.json",
            stringtestutil::dedent(&format!(
                r#"
                {{
                    "compilerOptions": {{
                        "target": "es5",
                        "declaration": true,
                        "outDir": "obj",
                        "incremental": true,
                        "isolatedModules": {isolated_modules},
                    }},
                }}"#
            )),
        ),
    ] {
        files.insert(path.to_owned(), text.into_map_file(SystemTime::UNIX_EPOCH));
    }
    files
}

fn remove_bar_param_type(sys: &mut TestSys) {
    sys.replace_file_text("/home/src/workspaces/project/bar.ts", "param: string", "");
}

fn restore_bar_param_type(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/bar.ts",
        "foobar()",
        "foobar(param: string)",
    );
}

fn fix_lazy_index_bar_call(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/lazyIndex.ts",
        r#"bar("hello")"#,
        "bar()",
    );
}

fn build_inferred_type_from_monorepo_reference_map() -> FileMap {
    let mut files = file_map(&[
        (
            "/home/src/workspaces/solution/package.json",
            &stringtestutil::dedent(
                r#"
                {
                    "name": "tsgo-monorepo-issue",
                    "private": true,
                    "workspaces": ["packages/*"]
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "files": [],
                    "include": [],
                    "references": [
                        { "path": "packages/package-a" },
                        { "path": "packages/package-b" },
                        { "path": "packages/package-c" }
                    ]
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-c/package.json",
            &stringtestutil::dedent(
                r#"
                {
                    "name": "package-c",
                    "version": "1.0.0",
                    "private": true,
                    "type": "module",
                    "main": "./src/index.ts",
                    "types": "./src/index.ts",
                    "exports": {
                        ".": "./src/index.ts"
                    }
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-c/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                        "declaration": true,
                        "emitDeclarationOnly": true,
                        "module": "ESNext",
                        "moduleResolution": "Bundler",
                        "target": "ES2022",
                        "outDir": "./out",
                        "rootDir": "./src"
                    },
                    "include": ["src/**/*"]
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-c/src/index.ts",
            &stringtestutil::dedent(
                r#"
                export interface MyType {
                    id: string;
                    name: string;
                    enabled: boolean;
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-b/package.json",
            &stringtestutil::dedent(
                r#"
                {
                    "name": "package-b",
                    "version": "1.0.0",
                    "private": true,
                    "type": "module",
                    "main": "./src/index.ts",
                    "types": "./src/index.ts",
                    "exports": {
                        ".": "./src/index.ts"
                    },
                    "dependencies": {
                        "package-c": "workspace:*"
                    }
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-b/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                        "declaration": true,
                        "emitDeclarationOnly": true,
                        "module": "ESNext",
                        "moduleResolution": "Bundler",
                        "target": "ES2022",
                        "outDir": "./out",
                        "rootDir": "./src"
                    },
                    "include": ["src/**/*"],
                    "references": [{ "path": "../package-c" }]
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-b/src/index.ts",
            &stringtestutil::dedent(
                r#"
                import type { MyType } from "package-c";

                export function createThing(input: MyType): MyType {
                    return { ...input };
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-a/package.json",
            &stringtestutil::dedent(
                r#"
                {
                    "name": "package-a",
                    "version": "1.0.0",
                    "private": true,
                    "type": "module",
                    "main": "./src/index.ts",
                    "types": "./src/index.ts",
                    "exports": {
                        ".": "./src/index.ts"
                    },
                    "dependencies": {
                        "package-b": "workspace:*"
                    }
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-a/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                        "declaration": true,
                        "emitDeclarationOnly": true,
                        "module": "ESNext",
                        "moduleResolution": "Bundler",
                        "target": "ES2022",
                        "outDir": "./out",
                        "rootDir": "./src"
                    },
                    "include": ["src/**/*"],
                    "references": [{ "path": "../package-b" }]
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/packages/package-a/src/index.ts",
            &stringtestutil::dedent(
                r#"
                import { createThing } from "package-b";

                class MyClass {
                    public thing = createThing({ id: "1", name: "test", enabled: true });
                }

                export { MyClass };"#,
            ),
        ),
    ]);

    files.insert(
        "/home/src/workspaces/solution/node_modules/package-a".to_owned(),
        vfstest::symlink("/home/src/workspaces/solution/packages/package-a"),
    );
    files.insert(
        "/home/src/workspaces/solution/node_modules/package-b".to_owned(),
        vfstest::symlink("/home/src/workspaces/solution/packages/package-b"),
    );
    files.insert(
        "/home/src/workspaces/solution/node_modules/package-c".to_owned(),
        vfstest::symlink("/home/src/workspaces/solution/packages/package-c"),
    );
    files
}

fn build_javascript_project_emit_map() -> FileMap {
    let mut files = file_map(&[
        (
            "/home/src/workspaces/solution/common/nominal.js",
            &stringtestutil::dedent(
                r#"
                    /**
                     * @template T, Name
                     * @typedef {T & {[Symbol.species]: Name}} Nominal
                     */
                    module.exports = {};"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/common/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig.base.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "include": ["nominal.js"],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project/index.js",
            &stringtestutil::dedent(
                r#"
                    import { Nominal } from '../common/nominal';

                    /**
                     * @typedef {Nominal<string, 'MyNominal'>} MyNominal
                     */"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig.base.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "../common" },
                    ],
                    "include": ["./index.js"],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project-2/index.js",
            &stringtestutil::dedent(
                r#"
                    import { MyNominal } from '../sub-project/index';

                    const variable = {
                        key: /** @type {MyNominal} */('value'),
                    };

                    /**
                     * @return {keyof typeof variable}
                     */
                    export function getVar() {
                        return 'key';
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project-2/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig.base.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "../sub-project" },
                    ],
                    "include": ["./index.js"],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "./sub-project" },
                        { "path": "./sub-project-2" },
                    ],
                    "include": [],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/tsconfig.base.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "skipLibCheck": true,
                        "rootDir": "./",
                        "outDir": "../lib",
                        "allowJs": true,
                        "checkJs": true,
                        "declaration": true,
                    },
                }"#,
            ),
        ),
    ]);
    files.insert(
        format!("{TSC_LIB_PATH}/lib.d.ts"),
        tsc_default_lib_content()
            .replacen(
                "interface SymbolConstructor {",
                "interface SymbolConstructor {\n    readonly species: symbol;",
                1,
            )
            .into_map_file(SystemTime::UNIX_EPOCH),
    );
    files
}

fn build_javascript_project_emit_non_moved_json_map() -> FileMap {
    file_map(&[
        (
            "/home/src/workspaces/solution/common/obj.json",
            &stringtestutil::dedent(
                r#"
                {
                    "val": 42,
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/common/index.ts",
            &stringtestutil::dedent(
                r#"
                    import x = require("./obj.json");
                    export = x;"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/common/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig.base.json",
                    "compilerOptions": {
                        "outDir": null,
                        "composite": true,
                    },
                    "include": ["index.ts", "obj.json"],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project/index.js",
            &stringtestutil::dedent(
                r#"
                    import mod from '../common';

                    export const m = mod;"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig.base.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "../common" },
                    ],
                    "include": ["./index.js"],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project-2/index.js",
            &stringtestutil::dedent(
                r#"
                    import { m } from '../sub-project/index';

                    const variable = {
                        key: m,
                    };

                    export function getVar() {
                        return variable;
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/sub-project-2/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig.base.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "../sub-project" },
                    ],
                    "include": ["./index.js"],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "./sub-project" },
                        { "path": "./sub-project-2" },
                    ],
                    "include": [],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/tsconfig.base.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "skipLibCheck": true,
                        "rootDir": "./",
                        "outDir": "../out",
                        "allowJs": true,
                        "checkJs": true,
                        "resolveJsonModule": true,
                        "esModuleInterop": true,
                        "declaration": true,
                    },
                }"#,
            ),
        ),
    ])
}

fn build_late_bound_symbol_map() -> FileMap {
    file_map(&[
        (
            "/home/src/workspaces/project/src/globals.d.ts",
            &stringtestutil::dedent(
                r#"
                    interface SymbolConstructor {
                        (description?: string | number): symbol;
                    }
                    declare var Symbol: SymbolConstructor;"#,
            ),
        ),
        (
            "/home/src/workspaces/project/src/hkt.ts",
            "export interface HKT<T> { }",
        ),
        (
            "/home/src/workspaces/project/src/main.ts",
            &stringtestutil::dedent(
                r#"
                    import { HKT } from "./hkt";

                    const sym = Symbol();

                    declare module "./hkt" {
                        interface HKT<T> {
                            [sym]: { a: T }
                        }
                    }
                    const x = 10;
                    type A = HKT<number>[typeof sym];"#,
            ),
        ),
        (
            "/home/src/workspaces/project/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "rootDir": "src",
                        "incremental": true,
                    },
                }"#,
            ),
        ),
    ])
}

fn remove_late_bound_symbol_unrelated_const(sys: &mut TestSys) {
    sys.replace_file_text(
        "/home/src/workspaces/project/src/main.ts",
        "const x = 10;",
        "",
    );
}

fn append_late_bound_symbol_unrelated_const(sys: &mut TestSys) {
    sys.append_file("/home/src/workspaces/project/src/main.ts", "const x = 10;");
}

fn build_module_specifiers_synthesized_resolve_map() -> FileMap {
    let mut files = file_map(&[
        (
            "/home/src/workspaces/packages/solution/common/nominal.ts",
            &stringtestutil::dedent(
                r#"
                    export declare type Nominal<T, Name extends string> = T & {
                        [Symbol.species]: Name;
                    };"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/solution/common/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../../tsconfig.base.json",
                    "compilerOptions": {
                        "composite": true
                    },
                    "include": ["nominal.ts"]
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/solution/sub-project/index.ts",
            &stringtestutil::dedent(
                r#"
                    import { Nominal } from '../common/nominal';

                    export type MyNominal = Nominal<string, 'MyNominal'>;"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/solution/sub-project/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "extends": "../../tsconfig.base.json",
                        "compilerOptions": {
                            "composite": true
                        },
                        "references": [
                            { "path": "../common" }
                        ],
                        "include": ["./index.ts"]
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/solution/sub-project-2/index.ts",
            &stringtestutil::dedent(
                r#"
                    import { MyNominal } from '../sub-project/index';

                    const variable = {
                        key: 'value' as MyNominal,
                    };

                    export function getVar(): keyof typeof variable {
                        return 'key';
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/solution/sub-project-2/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "extends": "../../tsconfig.base.json",
                        "compilerOptions": {
                            "composite": true
                        },
                        "references": [
                            { "path": "../sub-project" }
                        ],
                        "include": ["./index.ts"]
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/solution/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "compilerOptions": {
                            "composite": true
                        },
                        "references": [
                            { "path": "./sub-project" },
                            { "path": "./sub-project-2" }
                        ],
                        "include": []
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/tsconfig.base.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "compilerOptions": {
                            "skipLibCheck": true,
                            "rootDir": "./",
                            "outDir": "lib"
                        }
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "compilerOptions": {
                            "composite": true
                        },
                        "references": [
                            { "path": "./solution" },
                        ],
                        "include": [],
                    }"#,
            ),
        ),
    ]);
    files.insert(
        format!("{TSC_LIB_PATH}/lib.d.ts"),
        tsc_default_lib_content()
            .replacen(
                "interface SymbolConstructor {",
                "interface SymbolConstructor {\n    readonly species: symbol;",
                1,
            )
            .into_map_file(SystemTime::UNIX_EPOCH),
    );
    files
}

fn build_module_specifiers_across_projects_map() -> FileMap {
    let mut files = file_map(&[
        (
            "/home/src/workspaces/packages/src-types/index.ts",
            &stringtestutil::dedent(
                r#"
                    export * from './dogconfig.js';"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-types/dogconfig.ts",
            &stringtestutil::dedent(
                r#"
                    export interface DogConfig {
                        name: string;
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-dogs/index.ts",
            &stringtestutil::dedent(
                r#"
                    export * from 'src-types';
                    export * from './lassie/lassiedog.js';"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-dogs/dogconfig.ts",
            &stringtestutil::dedent(
                r#"
                    import { DogConfig } from 'src-types';

                    export const DOG_CONFIG: DogConfig = {
                        name: 'Default dog',
                    };"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-dogs/dog.ts",
            &stringtestutil::dedent(
                r#"
                    import { DogConfig } from 'src-types';
                    import { DOG_CONFIG } from './dogconfig.js';

                    export abstract class Dog {

                        public static getCapabilities(): DogConfig {
                            return DOG_CONFIG;
                        }
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-dogs/lassie/lassiedog.ts",
            &stringtestutil::dedent(
                r#"
                    import { Dog } from '../dog.js';
                    import { LASSIE_CONFIG } from './lassieconfig.js';

                    export class LassieDog extends Dog {
                        protected static getDogConfig = () => LASSIE_CONFIG;
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-dogs/lassie/lassieconfig.ts",
            &stringtestutil::dedent(
                r#"
                    import { DogConfig } from 'src-types';

                    export const LASSIE_CONFIG: DogConfig = { name: 'Lassie' };"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/tsconfig-base.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "compilerOptions": {
                            "declaration": true,
                            "module": "node16",
                        },
                    }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-types/package.json",
            &stringtestutil::dedent(
                r#"
                {
                    "type": "module",
                    "exports": "./index.js"
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-dogs/package.json",
            &stringtestutil::dedent(
                r#"
                {
                    "type": "module",
                    "exports": "./index.js"
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-types/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig-base.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "include": [
                        "**/*",
                    ],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/packages/src-dogs/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../tsconfig-base.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "../src-types" },
                    ],
                    "include": [
                        "**/*",
                    ],
                }"#,
            ),
        ),
    ]);
    files.insert(
        "/home/src/workspaces/packages/src-types/node_modules".to_owned(),
        vfstest::symlink("/home/src/workspaces/packages"),
    );
    files.insert(
        "/home/src/workspaces/packages/src-dogs/node_modules".to_owned(),
        vfstest::symlink("/home/src/workspaces/packages"),
    );
    files
}

struct TscOutputPathScenario {
    sub_scenario: String,
    files: FileMap,
    expected_dts_names: Vec<String>,
}

fn tsc_output_path_scenario(
    sub_scenario: &str,
    files: FileMap,
    expected_dts_names: Vec<&str>,
) -> TscOutputPathScenario {
    TscOutputPathScenario {
        sub_scenario: sub_scenario.to_owned(),
        files,
        expected_dts_names: expected_dts_names.into_iter().map(str::to_owned).collect(),
    }
}

fn run_output_paths(t: &mut dyn TestingT, scenario: TscOutputPathScenario) {
    t.helper();
    let input = tsc_input_with_edits(
        &scenario.sub_scenario,
        scenario.files,
        vec!["-b", "-v"],
        vec![
            no_change(),
            edit_command(
                "Normal build without change, that does not block emit on error to show files that get emitted",
                vec!["-p", "/home/src/workspaces/project/tsconfig.json"],
            ),
        ],
    );
    input.run(t, "outputPaths");
    t.run(
        &format!("GetOutputFileNames/{}", scenario.sub_scenario),
        &mut |t| {
            t.parallel();
            let sys = new_test_sys(&input, false);
            let system = sys.clone_system();
            let compiler_options = core::CompilerOptions::default();
            let (config, _) = tsoptions::get_parsed_command_line_of_config_file(
                "/home/src/workspaces/project/tsconfig.json",
                Some(&compiler_options),
                None,
                &system,
                None,
            );
            let config = config.unwrap_or_else(|| panic!("missing parsed tsconfig"));
            assert_eq!(config.get_output_file_names(), scenario.expected_dts_names);
        },
    );
}

fn program_updates_message_to_message2(sys: &mut TestSys) {
    sys.replace_file_text_all(
        "/user/username/projects/sample1/Library/library.ts",
        "message",
        "message2",
    );
}

fn program_updates_message2_to_message(sys: &mut TestSys) {
    sys.replace_file_text_all(
        "/user/username/projects/sample1/Library/library.ts",
        "message2",
        "message",
    );
}

fn program_updates_fix_file_with_error(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/solution/app/fileWithError.ts",
        "private p = 12",
        "",
    );
}

fn program_updates_change_file_without_error(sys: &mut TestSys) {
    sys.replace_file_text_all(
        "/user/username/projects/solution/app/fileWithoutError.ts",
        "myClass",
        "myClass2",
    );
}

fn program_updates_introduce_file_with_error(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/solution/app/fileWithError.ts",
        &stringtestutil::dedent(
            r#"
            export var myClassWithError = class {
                tags() { }
                private p = 12
            };"#,
        ),
    );
}

fn program_updates_set_no_unused_parameters_false(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/myproject/tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "noUnusedParameters": false,
                },
            }"#,
        ),
    );
}

fn program_updates_modify_alpha_config(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/project/alpha.tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "strict": true
                }
            }"#,
        ),
    );
}

fn program_updates_change_bravo_config(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/project/bravo.tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "extends": "./alpha.tsconfig.json",
                "compilerOptions": { "strict": false }
            }"#,
        ),
    );
}

fn program_updates_project2_extends_alpha(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/project/project2.tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "extends": "./alpha.tsconfig.json",
                "files": ["other.ts"]
            }"#,
        ),
    );
}

fn program_updates_alpha_config_empty(sys: &mut TestSys) {
    sys.write_file_no_error("/user/username/projects/project/alpha.tsconfig.json", "{}");
}

fn program_updates_modify_extends_config_file2(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/project/extendsConfig2.tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "compilerOptions": { "strictNullChecks": true }
            }"#,
        ),
    );
}

fn program_updates_modify_project3(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/project/project3.tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "extends": ["./extendsConfig1.tsconfig.json", "./extendsConfig2.tsconfig.json"],
                "compilerOptions": { "composite": false },
                "files": ["other2.ts"],
            }"#,
        ),
    );
}

fn program_updates_delete_extends_config_file2(sys: &mut TestSys) {
    sys.remove_no_error("/user/username/projects/project/extendsConfig2.tsconfig.json");
}

fn program_updates_remove_project2_from_base_config(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/project/tsconfig.json",
        &stringtestutil::dedent(
            r#"
            {
                "references": [
                    {
                        "path": "./project1.tsconfig.json",
                    },
                ],
                "files": [],
            }"#,
        ),
    );
}

fn program_updates_append_bar_to_lib_foo(sys: &mut TestSys) {
    sys.append_file("/home/src/workspaces/project/lib/foo.ts", "const Bar = 10;");
}

fn program_updates_referenced_project_error_map() -> FileMap {
    file_map(&[
        (
            "/user/username/projects/sample1/Library/tsconfig.json",
            "{ \n    \"compilerOptions\": {\n        \"composite\": true\n    }\n}",
        ),
        (
            "/user/username/projects/sample1/Library/library.ts",
            &stringtestutil::dedent(
                r#"
                interface SomeObject
                {
                    message: string;
                }

                export function createSomeObject(): SomeObject
                {
                    return {
                        message: "new Object"
                    };
                }"#,
            ),
        ),
        (
            "/user/username/projects/sample1/App/tsconfig.json",
            "{ \n    \"references\": [{ \"path\": \"../Library\" }]\n}",
        ),
        (
            "/user/username/projects/sample1/App/app.ts",
            &stringtestutil::dedent(
                r#"
                import { createSomeObject } from "../Library/library";
                createSomeObject().message;"#,
            ),
        ),
    ])
}

fn program_updates_declaration_emit_errors_map(with_error: bool) -> FileMap {
    file_map(&[
        (
            "/user/username/projects/solution/app/fileWithError.ts",
            &stringtestutil::dedent(if with_error {
                r#"
                export var myClassWithError = class {
                    tags() { }
                    private p = 12
                };"#
            } else {
                r#"
                export var myClassWithError = class {
                    tags() { }

                };"#
            }),
        ),
        (
            "/user/username/projects/solution/app/fileWithoutError.ts",
            "export class myClass { }",
        ),
        (
            "/user/username/projects/solution/app/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true
                    }
                }"#,
            ),
        ),
    ])
}

fn program_updates_extended_source_files_map() -> FileMap {
    file_map(&[
        (
            "/user/username/projects/project/commonFile1.ts",
            "let x = 1",
        ),
        (
            "/user/username/projects/project/commonFile2.ts",
            "let y = 1",
        ),
        ("/user/username/projects/project/alpha.tsconfig.json", "{}"),
        (
            "/user/username/projects/project/project1.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "./alpha.tsconfig.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "files": ["commonFile1.ts", "commonFile2.ts"],
                }"#,
            ),
        ),
        (
            "/user/username/projects/project/bravo.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "./alpha.tsconfig.json",
                }"#,
            ),
        ),
        ("/user/username/projects/project/other.ts", "let z = 0;"),
        (
            "/user/username/projects/project/project2.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "./bravo.tsconfig.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "files": ["other.ts"],
                }"#,
            ),
        ),
        ("/user/username/projects/project/other2.ts", "let k = 0;"),
        (
            "/user/username/projects/project/extendsConfig1.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                    },
                }"#,
            ),
        ),
        (
            "/user/username/projects/project/extendsConfig2.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "strictNullChecks": false,
                    },
                }"#,
            ),
        ),
        (
            "/user/username/projects/project/extendsConfig3.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "noImplicitAny": true,
                    },
                }"#,
            ),
        ),
        (
            "/user/username/projects/project/project3.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": [
                        "./extendsConfig1.tsconfig.json",
                        "./extendsConfig2.tsconfig.json",
                        "./extendsConfig3.tsconfig.json",
                    ],
                    "compilerOptions": {
                        "composite": false,
                    },
                    "files": ["other2.ts"],
                }"#,
            ),
        ),
    ])
}

fn program_updates_project_with_extended_config_removed_map() -> FileMap {
    file_map(&[
        (
            "/user/username/projects/project/commonFile1.ts",
            "let x = 1",
        ),
        (
            "/user/username/projects/project/commonFile2.ts",
            "let y = 1",
        ),
        (
            "/user/username/projects/project/alpha.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "strict": true,
                    },
                }"#,
            ),
        ),
        (
            "/user/username/projects/project/project1.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "./alpha.tsconfig.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "files": ["commonFile1.ts", "commonFile2.ts"],
                }"#,
            ),
        ),
        (
            "/user/username/projects/project/bravo.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "strict": true,
                    },
                }"#,
            ),
        ),
        ("/user/username/projects/project/other.ts", "let z = 0;"),
        (
            "/user/username/projects/project/project2.tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "./bravo.tsconfig.json",
                    "compilerOptions": {
                        "composite": true,
                    },
                    "files": ["other.ts"],
                }"#,
            ),
        ),
        (
            "/user/username/projects/project/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "references": [
                        {
                            "path": "./project1.tsconfig.json",
                        },
                        {
                            "path": "./project2.tsconfig.json",
                        },
                    ],
                    "files": [],
                }"#,
            ),
        ),
    ])
}

fn program_updates_root_source_from_project_reference_map(root_composite: bool) -> FileMap {
    let root_tsconfig = if root_composite {
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "composite": true,
                },
                "references": [ { "path": "./lib" } ]
            }"#,
        )
    } else {
        stringtestutil::dedent(
            r#"
            {
                "references": [ { "path": "./lib" } ]
            }"#,
        )
    };
    file_map(&[
        (
            "/home/src/workspaces/project/lib/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                        "outDir": "./dist"
                    }
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/project/lib/foo.ts",
            "export const FOO: string = 'THEFOOEXPORT';",
        ),
        ("/home/src/workspaces/project/tsconfig.json", &root_tsconfig),
        (
            "/home/src/workspaces/project/index.ts",
            r#"import { FOO } from "./lib/foo";"#,
        ),
    ])
}

fn projects_building_append_pkg0_local(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/myproject/pkg0/index.ts",
        "const someConst2 = 10;",
    );
}

fn projects_building_append_pkg0_export(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/myproject/pkg0/index.ts",
        "export const someConst = 10;",
    );
}

fn projects_building_edits() -> Vec<TscEdit> {
    vec![
        local_change("dts doesn't change", projects_building_append_pkg0_local),
        no_change(),
        local_change("dts change", projects_building_append_pkg0_export),
        no_change(),
    ]
}

fn projects_building_add_package_files(files: &mut FileMap, index: usize) {
    files.insert(
        format!("/user/username/projects/myproject/pkg{index}/index.ts"),
        format!("export const pkg{index} = {index};").into_map_file(SystemTime::UNIX_EPOCH),
    );
    let tsconfig = if index > 0 {
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": { "composite": true },
                "references": [{ "path": "../pkg0" }],
            }"#,
        )
    } else {
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": { "composite": true },

            }"#,
        )
    };
    files.insert(
        format!("/user/username/projects/myproject/pkg{index}/tsconfig.json"),
        tsconfig.into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn projects_building_add_solution(files: &mut FileMap, count: usize) {
    let pkg_references = (0..count)
        .map(|i| format!(r#"{{ "path": "./pkg{i}" }}"#))
        .collect::<Vec<_>>()
        .join(",\n                    ");
    files.insert(
        "/user/username/projects/myproject/tsconfig.json".to_owned(),
        stringtestutil::dedent(&format!(
            r#"
            {{
                "compilerOptions": {{ "composite": true }},
                "references": [
                    {pkg_references}
                ]
            }}"#
        ))
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn projects_building_files(count: usize) -> FileMap {
    let mut files = FileMap::new();
    for index in 0..count {
        projects_building_add_package_files(&mut files, index);
    }
    projects_building_add_solution(&mut files, count);
    files
}

fn projects_building_test_cases(pkg_count: usize, builders: usize) -> Vec<TscInput> {
    let builders_str = builders.to_string();
    vec![
        tsc_input_with_cwd_edits(
            &format!("when there are {pkg_count} projects in a solution"),
            projects_building_files(pkg_count),
            "/user/username/projects/myproject",
            vec!["-b", "-v"],
            projects_building_edits(),
        ),
        tsc_input_with_cwd_edits(
            &format!(
                "when there are {pkg_count} projects in a solution with --builders {builders}"
            ),
            projects_building_files(pkg_count),
            "/user/username/projects/myproject",
            vec!["-b", "-v", "--builders", &builders_str],
            projects_building_edits(),
        ),
        tsc_input_with_cwd_edits(
            &format!("when there are {pkg_count} projects in a solution"),
            projects_building_files(pkg_count),
            "/user/username/projects/myproject",
            vec!["-b", "-w", "-v"],
            projects_building_edits(),
        ),
        tsc_input_with_cwd_edits(
            &format!(
                "when there are {pkg_count} projects in a solution with --builders {builders}"
            ),
            projects_building_files(pkg_count),
            "/user/username/projects/myproject",
            vec!["-b", "-w", "-v", "--builders", &builders_str],
            projects_building_edits(),
        ),
    ]
}

enum ProjectReferenceWithRootDirInParentVariant {
    Default,
    NoRootDirInBase,
    SameTsBuildInfo,
    SameTsBuildInfoWithoutIncremental,
    TsBuildInfoDiffer,
}

fn build_project_reference_with_root_dir_in_parent_file_map(
    variant: ProjectReferenceWithRootDirInParentVariant,
) -> FileMap {
    let default_base_config = stringtestutil::dedent(
        r#"
        {
            "compilerOptions": {
                "composite": true,
                "declaration": true,
                "rootDir": "./src/",
                "outDir": "./dist/",
                "skipDefaultLibCheck": true,
            },
            "exclude": [
                "node_modules",
            ],
        }"#,
    );
    let base_config = if matches!(
        variant,
        ProjectReferenceWithRootDirInParentVariant::NoRootDirInBase
    ) {
        default_base_config.replacen(r#""rootDir": "./src/","#, "", 1)
    } else {
        default_base_config
    };
    let mut files = file_map(&[
        (
            "/home/src/workspaces/solution/src/main/a.ts",
            &stringtestutil::dedent(
                r#"
                import { b } from './b';
                const a = b;"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/src/main/b.ts",
            &stringtestutil::dedent(
                r#"
                export const b = 0;"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/src/main/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../../tsconfig.base.json",
                    "references": [
                        { "path": "../other" },
                    ],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/src/other/other.ts",
            &stringtestutil::dedent(
                r#"
                export const Other = 0;"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/src/other/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../../tsconfig.base.json",
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/tsconfig.base.json",
            &base_config,
        ),
    ]);

    match variant {
        ProjectReferenceWithRootDirInParentVariant::SameTsBuildInfo => {
            files.insert(
                "/home/src/workspaces/solution/src/main/tsconfig.json".to_owned(),
                stringtestutil::dedent(
                    r#"
                    {
                        "compilerOptions": { "composite": true, "outDir": "../../dist/" },
                        "references": [{ "path": "../other" }]
                    }"#,
                )
                .into_map_file(SystemTime::UNIX_EPOCH),
            );
            files.insert(
                "/home/src/workspaces/solution/src/other/tsconfig.json".to_owned(),
                stringtestutil::dedent(
                    r#"
                    {
                        "compilerOptions": { "composite": true, "outDir": "../../dist/" },
                    }"#,
                )
                .into_map_file(SystemTime::UNIX_EPOCH),
            );
        }
        ProjectReferenceWithRootDirInParentVariant::SameTsBuildInfoWithoutIncremental => {
            files.insert(
                "/home/src/workspaces/solution/src/main/tsconfig.json".to_owned(),
                stringtestutil::dedent(
                    r#"
                    {
                        "compilerOptions": { "outDir": "../../dist/" },
                        "references": [{ "path": "../other" }]
                    }"#,
                )
                .into_map_file(SystemTime::UNIX_EPOCH),
            );
            files.insert(
                "/home/src/workspaces/solution/src/other/tsconfig.json".to_owned(),
                stringtestutil::dedent(
                    r#"
                    {
                        "compilerOptions": { "composite": true, "outDir": "../../dist/" },
                    }"#,
                )
                .into_map_file(SystemTime::UNIX_EPOCH),
            );
        }
        ProjectReferenceWithRootDirInParentVariant::TsBuildInfoDiffer => {
            files.remove("/home/src/workspaces/solution/src/main/tsconfig.json");
            files.remove("/home/src/workspaces/solution/src/other/tsconfig.json");
            files.insert(
                "/home/src/workspaces/solution/src/main/tsconfig.main.json".to_owned(),
                stringtestutil::dedent(
                    r#"
                    {
                        "compilerOptions": { "composite": true, "outDir": "../../dist/" },
                        "references": [{ "path": "../other/tsconfig.other.json" }]
                    }"#,
                )
                .into_map_file(SystemTime::UNIX_EPOCH),
            );
            files.insert(
                "/home/src/workspaces/solution/src/other/tsconfig.other.json".to_owned(),
                stringtestutil::dedent(
                    r#"
                    {
                        "compilerOptions": { "composite": true, "outDir": "../../dist/" },
                    }"#,
                )
                .into_map_file(SystemTime::UNIX_EPOCH),
            );
        }
        ProjectReferenceWithRootDirInParentVariant::Default
        | ProjectReferenceWithRootDirInParentVariant::NoRootDirInBase => {}
    }

    files
}

fn build_reexport_file_map() -> FileMap {
    file_map(&[
        (
            "/user/username/projects/reexport/src/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "files": [],
                    "include": [],
                    "references": [{ "path": "./pure" }, { "path": "./main" }],
                }"#,
            ),
        ),
        (
            "/user/username/projects/reexport/src/main/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "outDir": "../../out",
                        "rootDir": "../",
                    },
                    "include": ["**/*.ts"],
                    "references": [{ "path": "../pure" }],
                }"#,
            ),
        ),
        (
            "/user/username/projects/reexport/src/main/index.ts",
            &stringtestutil::dedent(
                r#"
                    import { Session } from "../pure";

                    export const session: Session = {
                        foo: 1
                    };
                "#,
            ),
        ),
        (
            "/user/username/projects/reexport/src/pure/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                        "outDir": "../../out",
                        "rootDir": "../",
                    },
                    "include": ["**/*.ts"],
                }"#,
            ),
        ),
        (
            "/user/username/projects/reexport/src/pure/index.ts",
            r#"export * from "./session";"#,
        ),
        (
            "/user/username/projects/reexport/src/pure/session.ts",
            &stringtestutil::dedent(
                r#"
                    export interface Session {
                        foo: number;
                        // bar: number;
                    }
                "#,
            ),
        ),
    ])
}

fn introduce_reexport_session_error(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/reexport/src/pure/session.ts",
        "// ",
        "",
    );
}

fn fix_reexport_session_error(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/reexport/src/pure/session.ts",
        "bar: ",
        "// bar: ",
    );
}

struct BuildResolveJsonModuleScenario {
    sub_scenario: &'static str,
    tsconfig_files: &'static str,
    additional_compiler_options: &'static str,
    skip_outdir: bool,
    modify_files: Option<fn(&mut FileMap)>,
    edits: ResolveJsonModuleEdits,
}

#[derive(Clone, Copy)]
enum ResolveJsonModuleEdits {
    None,
    NoChangeOnly,
}

impl ResolveJsonModuleEdits {
    fn into_edits(self) -> Vec<TscEdit> {
        match self {
            ResolveJsonModuleEdits::None => Vec::new(),
            ResolveJsonModuleEdits::NoChangeOnly => no_change_only_edit(),
        }
    }
}

fn replace_file_map_text(files: &mut FileMap, path: &str, old_text: &str, new_text: &str) {
    let file = files
        .get_mut(path)
        .unwrap_or_else(|| panic!("missing file {path}"));
    let text = String::from_utf8(file.data.to_vec())
        .unwrap_or_else(|err| panic!("non-utf8 file {path}: {err}"));
    let text = text.replacen(old_text, new_text, 1);
    file.data = text.as_bytes().into();
    file.text = Some(text.into());
}

fn resolve_json_module_json_not_in_root_dir(files: &mut FileMap) {
    let text = files
        .remove("/home/src/workspaces/solution/project/src/hello.json")
        .unwrap_or_else(|| {
            panic!("missing file /home/src/workspaces/solution/project/src/hello.json")
        });
    files.insert(
        "/home/src/workspaces/solution/project/hello.json".to_owned(),
        text,
    );
    replace_file_map_text(
        files,
        "/home/src/workspaces/solution/project/src/index.ts",
        "./hello.json",
        "../hello.json",
    );
}

fn resolve_json_module_json_outside_config_directory(files: &mut FileMap) {
    let text = files
        .remove("/home/src/workspaces/solution/project/src/hello.json")
        .unwrap_or_else(|| {
            panic!("missing file /home/src/workspaces/solution/project/src/hello.json")
        });
    files.insert("/home/src/workspaces/solution/hello.json".to_owned(), text);
    replace_file_map_text(
        files,
        "/home/src/workspaces/solution/project/src/index.ts",
        "./hello.json",
        "../../hello.json",
    );
}

fn resolve_json_module_json_file_name_matches_ts_file(files: &mut FileMap) {
    let text = files
        .remove("/home/src/workspaces/solution/project/src/hello.json")
        .unwrap_or_else(|| {
            panic!("missing file /home/src/workspaces/solution/project/src/hello.json")
        });
    files.insert(
        "/home/src/workspaces/solution/project/src/index.json".to_owned(),
        text,
    );
    replace_file_map_text(
        files,
        "/home/src/workspaces/solution/project/src/index.ts",
        "./hello.json",
        "./index.json",
    );
}

fn build_resolve_json_module_file_map(
    composite: bool,
    scenario: &BuildResolveJsonModuleScenario,
) -> FileMap {
    let out_dir_str = if scenario.skip_outdir {
        String::new()
    } else {
        r#"                        "outDir": "dist","#.to_string()
    };
    let additional_compiler_options = if scenario.additional_compiler_options.is_empty() {
        String::new()
    } else {
        format!(
            "                        {}",
            scenario.additional_compiler_options
        )
    };
    let mut files = file_map(&[
        (
            "/home/src/workspaces/solution/project/src/hello.json",
            &stringtestutil::dedent(
                r#"
                {
                    "hello": "world"
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/project/src/index.ts",
            &stringtestutil::dedent(
                r#"
                    import hello from "./hello.json"
                    export default hello.hello
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/project/tsconfig.json",
            &stringtestutil::dedent(&format!(
                r#"
                {{
                    "compilerOptions": {{
                        "composite": {composite},
                        "module": "commonjs",
                        "resolveJsonModule": true,
                        "esModuleInterop": true,
                        "allowSyntheticDefaultImports": true,
{out_dir_str}
                        "skipDefaultLibCheck": true,
{additional_compiler_options}
                    }},
                    {tsconfig_files}
                }}"#,
                tsconfig_files = scenario.tsconfig_files,
            )),
        ),
    ]);
    if let Some(modify_files) = scenario.modify_files {
        modify_files(&mut files);
    }
    files
}

fn build_resolve_json_module_test_cases(
    scenarios: &[BuildResolveJsonModuleScenario],
) -> Vec<TscInput> {
    let mut test_cases = Vec::with_capacity(scenarios.len() * 2);
    for scenario in scenarios {
        test_cases.push(tsc_input_with_cwd_edits(
            scenario.sub_scenario,
            build_resolve_json_module_file_map(true, scenario),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "project",
                "--v",
                "--explainFiles",
                "--listEmittedFiles",
            ],
            scenario.edits.into_edits(),
        ));
        test_cases.push(tsc_input_with_cwd_edits(
            &format!("{} non-composite", scenario.sub_scenario),
            build_resolve_json_module_file_map(false, scenario),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "project",
                "--v",
                "--explainFiles",
                "--listEmittedFiles",
            ],
            scenario.edits.into_edits(),
        ));
    }
    test_cases
}

fn build_resolve_json_module_scenarios() -> Vec<BuildResolveJsonModuleScenario> {
    vec![
        BuildResolveJsonModuleScenario {
            sub_scenario: "include only",
            tsconfig_files: r#""include": [ "src/**/*" ],"#,
            additional_compiler_options: "",
            skip_outdir: false,
            modify_files: None,
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "include only without outDir",
            tsconfig_files: r#""include": [ "src/**/*" ],"#,
            additional_compiler_options: "",
            skip_outdir: true,
            modify_files: None,
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "include only with json not in rootDir",
            tsconfig_files: r#""include": [ "src/**/*" ],"#,
            additional_compiler_options: r#""rootDir": "src","#,
            skip_outdir: false,
            modify_files: Some(resolve_json_module_json_not_in_root_dir),
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "include only with json without rootDir but outside configDirectory",
            tsconfig_files: r#""include": [ "src/**/*" ],"#,
            additional_compiler_options: "",
            skip_outdir: false,
            modify_files: Some(resolve_json_module_json_outside_config_directory),
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "include of json along with other include",
            tsconfig_files: r#""include": [ "src/**/*", "src/**/*.json" ],"#,
            additional_compiler_options: "",
            skip_outdir: false,
            modify_files: None,
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "include of json along with other include and file name matches ts file",
            tsconfig_files: r#""include": [ "src/**/*", "src/**/*.json" ],"#,
            additional_compiler_options: "",
            skip_outdir: false,
            modify_files: Some(resolve_json_module_json_file_name_matches_ts_file),
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "files containing json file",
            tsconfig_files: r#""files": [ "src/index.ts", "src/hello.json", ],"#,
            additional_compiler_options: "",
            skip_outdir: false,
            modify_files: None,
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "include and files",
            tsconfig_files: r#""files": [ "src/hello.json" ], "include": [ "src/**/*" ],"#,
            additional_compiler_options: "",
            skip_outdir: false,
            modify_files: None,
            edits: ResolveJsonModuleEdits::None,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "sourcemap",
            tsconfig_files: r#""files": [ "src/index.ts", "src/hello.json", ],"#,
            additional_compiler_options: r#""sourceMap": true,"#,
            skip_outdir: false,
            modify_files: None,
            edits: ResolveJsonModuleEdits::NoChangeOnly,
        },
        BuildResolveJsonModuleScenario {
            sub_scenario: "without outDir",
            tsconfig_files: r#""files": [ "src/index.ts", "src/hello.json", ],"#,
            additional_compiler_options: "",
            skip_outdir: true,
            modify_files: None,
            edits: ResolveJsonModuleEdits::NoChangeOnly,
        },
    ]
}

fn build_resolve_json_module_project_reference_file_map() -> FileMap {
    file_map(&[
        (
            "/home/src/workspaces/solution/project/strings/foo.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "foo": "bar baz"
                    }
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/project/strings/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "extends": "../tsconfig.json",
                        "include": ["foo.json"],
                        "references": [],
                    }
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/project/main/index.ts",
            &stringtestutil::dedent(
                r#"
                    import { foo } from '../strings/foo.json';
                    console.log(foo);
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/project/main/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "extends": "../tsconfig.json",
                        "include": [
                            "./**/*.ts",
                        ],
                        "references": [{
                            "path": "../strings/tsconfig.json",
                        }],
                    }
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/project/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                    {
                        "compilerOptions": {
                            "target": "es5",
                            "module": "commonjs",
                            "rootDir": "./",
                            "composite": true,
                            "resolveJsonModule": true,
                            "strict": true,
                            "esModuleInterop": true,
                        },
                        "references": [
                            { "path": "./strings/tsconfig.json" },
                            { "path": "./main/tsconfig.json" },
                        ],
                        "files": [],
                    }
                "#,
            ),
        ),
    ])
}

fn build_roots_from_project_referenced_project_file_map(server_first: bool) -> FileMap {
    let include = if server_first {
        r#""src/**/*.ts", "../shared/src/**/*.ts""#
    } else {
        r#""../shared/src/**/*.ts", "src/**/*.ts""#
    };
    file_map(&[
        (
            "/home/src/workspaces/solution/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                    },
                    "references": [
                        { "path": "projects/server" },
                        { "path": "projects/shared" },
                    ],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/projects/shared/src/myClass.ts",
            "export class MyClass { }",
        ),
        (
            "/home/src/workspaces/solution/projects/shared/src/logging.ts",
            &stringtestutil::dedent(
                r#"
                    export function log(str: string) {
                        console.log(str);
                    }
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/projects/shared/src/random.ts",
            &stringtestutil::dedent(
                r#"
                    export function randomFn(str: string) {
                        console.log(str);
                    }
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/projects/shared/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "extends": "../../tsconfig.json",
                    "compilerOptions": {
                        "outDir": "./dist",
                    },
                    "include": ["src/**/*.ts"],
                }"#,
            ),
        ),
        (
            "/home/src/workspaces/solution/projects/server/src/server.ts",
            &stringtestutil::dedent(
                r#"
                    import { MyClass } from ':shared/myClass.js';
                    console.log('Hello, world!');
                "#,
            ),
        ),
        (
            "/home/src/workspaces/solution/projects/server/tsconfig.json",
            &stringtestutil::dedent(&format!(
                r#"
                {{
                    "extends": "../../tsconfig.json",
                    "compilerOptions": {{
                        "rootDir": "..",
                        "outDir": "./dist",
                        "paths": {{
                            ":shared/*": ["./src/../../shared/src/*"],
                        }},
                    }},
                    "include": [ {include} ],
                    "references": [
                        {{ "path": "../shared" }},
                    ],
                }}"#
            )),
        ),
    ])
}

fn delete_project_file1_outputs(sys: &mut TestSys) {
    sys.remove_no_error("/home/src/workspaces/project/file1.ts");
    sys.remove_no_error("/home/src/workspaces/project/file1.js");
    sys.remove_no_error("/home/src/workspaces/project/file1.d.ts");
}

fn roots_edit_logging_file(sys: &mut TestSys) {
    sys.append_file(
        "/home/src/workspaces/solution/projects/shared/src/logging.ts",
        "export const x = 10;",
    );
}

fn roots_delete_random_file(sys: &mut TestSys) {
    sys.remove_no_error("/home/src/workspaces/solution/projects/shared/src/random.ts");
}

fn build_roots_from_project_referenced_project_test_edits() -> Vec<TscEdit> {
    vec![
        no_change(),
        local_change("edit logging file", roots_edit_logging_file),
        no_change(),
        local_change("delete random file", roots_delete_random_file),
        no_change(),
    ]
}

fn build_sample_logic_config() -> String {
    stringtestutil::dedent(
        r#"
        {
            "compilerOptions": {
                "composite": true,
                "declaration": true,
                "sourceMap": true,
                "skipDefaultLibCheck": true,
            },
            "references": [
                { "path": "../core" },
            ],
        }"#,
    )
}

fn append_file_map_text(files: &mut FileMap, path: &str, text: &str) {
    let file = files
        .get_mut(path)
        .unwrap_or_else(|| panic!("missing file {path}"));
    let mut existing_text = String::from_utf8(file.data.to_vec())
        .unwrap_or_else(|err| panic!("non-utf8 file {path}: {err}"));
    existing_text.push_str(text);
    file.data = existing_text.as_bytes().into();
    file.text = Some(existing_text.into());
}

fn build_sample_file_map(modify: Option<fn(&mut FileMap)>) -> FileMap {
    let mut files = file_map(&[
        (
            "/user/username/projects/sample1/core/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "compilerOptions": {
                        "composite": true,
                        "declaration": true,
                        "declarationMap": true,
                        "skipDefaultLibCheck": true,
                    },
                }"#,
            ),
        ),
        (
            "/user/username/projects/sample1/core/index.ts",
            &stringtestutil::dedent(
                r#"
                    export const someString: string = "HELLO WORLD";
                    export function leftPad(s: string, n: number) { return s + n; }
                    export function multiply(a: number, b: number) { return a * b; }
                "#,
            ),
        ),
        (
            "/user/username/projects/sample1/core/some_decl.d.ts",
            "declare const dts: any;",
        ),
        (
            "/user/username/projects/sample1/core/anotherModule.ts",
            r#"export const World = "hello";"#,
        ),
        (
            "/user/username/projects/sample1/logic/tsconfig.json",
            &build_sample_logic_config(),
        ),
        (
            "/user/username/projects/sample1/logic/index.ts",
            &stringtestutil::dedent(
                r#"
                    import * as c from '../core/index';
                    export function getSecondsInDay() {
                        return c.multiply(10, 15);
                    }
                    import * as mod from '../core/anotherModule';
                    export const m = mod;
                "#,
            ),
        ),
        (
            "/user/username/projects/sample1/tests/tsconfig.json",
            &stringtestutil::dedent(
                r#"
                {
                    "references": [
                        { "path": "../core" },
                        { "path": "../logic" },
                    ],
                    "files": ["index.ts"],
                    "compilerOptions": {
                        "composite": true,
                        "declaration": true,
                        "skipDefaultLibCheck": true,
                    },
                }"#,
            ),
        ),
        (
            "/user/username/projects/sample1/tests/index.ts",
            &stringtestutil::dedent(
                r#"
                    import * as c from '../core/index';
                    import * as logic from '../logic/index';

                    c.leftPad("", 10);
                    logic.getSecondsInDay();

                    import * as mod from '../core/anotherModule';
                    export const m = mod;
                "#,
            ),
        ),
    ]);
    if let Some(modify) = modify {
        modify(&mut files);
    }
    files
}

fn build_sample_logic_config_out_dir(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/logic/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "composite": true,
                    "declaration": true,
                    "sourceMap": true,
                    "outDir": "outDir",
                },
                "references": [
                    { "path": "../core" },
                ],
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn build_sample_logic_config_declaration_dir(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/logic/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "composite": true,
                    "declaration": true,
                    "sourceMap": true,
                    "declarationDir": "out/decls",
                },
                "references": [
                    { "path": "../core" },
                ],
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn build_sample_core_not_composite(files: &mut FileMap) {
    replace_file_map_text(
        files,
        "/user/username/projects/sample1/core/tsconfig.json",
        r#""composite": true,"#,
        "",
    );
}

fn build_sample_add_core_error(files: &mut FileMap) {
    append_file_map_text(
        files,
        "/user/username/projects/sample1/core/index.ts",
        "multiply();",
    );
}

fn build_sample_tests_reference_logic_only_and_core_error(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/tests/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "references": [
                    { "path": "../logic" },
                ],
                "files": ["index.ts"],
                "compilerOptions": {
                    "composite": true,
                    "declaration": true,
                    "skipDefaultLibCheck": true,
                },
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
    build_sample_add_core_error(files);
}

fn build_sample_make_circular_references(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/core/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "composite": true,
                    "declaration": true
                },
                "references": [
                    { "path": "../tests", "circular": true }
                ],
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn build_sample_extended_config(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/tests/tsconfig.base.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "target": "es5"
                }
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
    replace_file_map_text(
        files,
        "/user/username/projects/sample1/tests/tsconfig.json",
        r#""references": ["#,
        r#""extends": "./tsconfig.base.json", "references": ["#,
    );
}

fn build_sample_logic_error(files: &mut FileMap) {
    replace_file_map_text(
        files,
        "/user/username/projects/sample1/logic/index.ts",
        "c.multiply(10, 15)",
        "c.muitply()",
    );
}

fn build_sample_logic_tsbuildinfo_file(files: &mut FileMap) {
    replace_file_map_text(
        files,
        "/user/username/projects/sample1/logic/tsconfig.json",
        r#""composite": true,"#,
        r#""composite": true,
    "tsBuildInfoFile": "ownFile.tsbuildinfo","#,
    );
}

fn build_sample_core_incremental_no_declaration(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/core/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "incremental": true,
                    "skipDefaultLibCheck": true,
                },
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn build_sample_core_target_esnext(files: &mut FileMap) {
    files.insert(
        get_test_lib_path_for("esnext.full"),
        r#"/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />"#
            .into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.insert(
        format!("{TSC_LIB_PATH}/lib.d.ts"),
        r#"/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />"#
            .into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.insert(
        "/user/username/projects/sample1/core/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "incremental": true,
                    "listFiles": true,
                    "listEmittedFiles": true,
                    "target": "esnext",
                },
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn build_sample_core_module_node18(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/core/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "incremental": true,
                    "module": "node18",
                },
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn build_sample_tests_es_module_interop_false(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/tests/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "references": [
                    { "path": "../core" },
                    { "path": "../logic" },
                ],
                "files": ["index.ts"],
                "compilerOptions": {
                    "composite": true,
                    "declaration": true,
                    "skipDefaultLibCheck": true,
                    "esModuleInterop": false,
                },
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn build_sample_missing_input_file(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/core/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                 "compilerOptions": { "composite": true },
                 "files": ["anotherModule.ts", "index.ts", "some_decl.d.ts"],
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.remove("/user/username/projects/sample1/core/anotherModule.ts");
}

fn build_sample_delete_logic_config(files: &mut FileMap) {
    files.remove("/user/username/projects/sample1/logic/tsconfig.json");
}

fn build_sample_core_out_dir(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/sample1/core/tsconfig.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "compilerOptions": {
                    "composite": true,
                    "outDir": "outDir"
                }
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

fn sample_fix_core_error(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/index.ts",
        "multiply();",
        "",
    );
}

fn sample_append_core_export_class(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/sample1/core/index.ts",
        "\nexport class someClass { }",
    );
}

fn sample_append_core_local_class(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/sample1/core/index.ts",
        "\nclass someClass2 { }",
    );
}

fn build_sample_core_change_edits() -> Vec<TscEdit> {
    vec![
        local_change(
            "incremental-declaration-changes",
            sample_append_core_export_class,
        ),
        local_change(
            "incremental-declaration-doesnt-change",
            sample_append_core_local_class,
        ),
        no_change(),
    ]
}

fn sample_revert_core_export_class(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/index.ts",
        "\nexport class someClass { }",
        "",
    );
}

fn sample_make_two_core_changes(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/sample1/core/index.ts",
        "\nexport class someClass { }",
    );
    sys.append_file(
        "/user/username/projects/sample1/core/index.ts",
        "\nexport class someClass2 { }",
    );
}

fn build_sample_watch_dts_changing_edits() -> Vec<TscEdit> {
    vec![
        local_change("Make change to core", sample_append_core_export_class),
        local_change("Revert core file", sample_revert_core_export_class),
        local_change("Make two changes", sample_make_two_core_changes),
    ]
}

fn sample_append_core_local_function(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/sample1/core/index.ts",
        "\nfunction foo() { }",
    );
}

fn build_sample_watch_non_dts_changing_edits() -> Vec<TscEdit> {
    vec![local_change(
        "Make local change to core",
        sample_append_core_local_function,
    )]
}

fn sample_write_core_new_file_const(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/sample1/core/newfile.ts",
        "export const newFileConst = 30;",
    );
}

fn sample_write_core_new_file_class(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/sample1/core/newfile.ts",
        "\nexport class someClass2 { }",
    );
}

fn build_sample_watch_new_file_edits() -> Vec<TscEdit> {
    vec![
        local_change(
            "Change to new File and build core",
            sample_write_core_new_file_const,
        ),
        local_change(
            "Change to new File and build core",
            sample_write_core_new_file_class,
        ),
    ]
}

fn sample_write_tests_index_const(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/sample1/tests/index.ts",
        "const m = 10;",
    );
}

fn sample_replace_core_hello_world(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/index.ts",
        "HELLO WORLD",
        "WELCOME PLANET",
    );
}

fn sample_rebuild_tests_target_es2020(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/tests/tsconfig.json",
        r#""composite": true"#,
        r#""composite": true, "target": "es2020""#,
    );
}

fn sample_touch_core_index(sys: &mut TestSys) {
    sys.fs_from_file_map()
        .chtimes(
            "/user/username/projects/sample1/core/index.ts",
            SystemTime::UNIX_EPOCH,
            sys.now(),
        )
        .unwrap_or_else(|err| panic!("failed to chtimes core/index.ts: {err}"));
}

fn sample_disable_declaration_map(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/tsconfig.json",
        r#""declarationMap": true,"#,
        r#""declarationMap": false,"#,
    );
}

fn sample_enable_declaration_map(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/tsconfig.json",
        r#""declarationMap": false,"#,
        r#""declarationMap": true,"#,
    );
}

fn sample_prepend_bad_tsbuildinfo(sys: &mut TestSys) {
    if !sys.for_incremental_correctness {
        sys.prepend_file(
            "/home/src/workspaces/project/tsconfig.tsbuildinfo",
            "Some random string",
        );
        sys.replace_file_text(
            "/home/src/workspaces/project/tsconfig.tsbuildinfo",
            &format!(r#""version":"{}""#, core::version()),
            &format!(r#""version":"{}""#, harnessutil::FAKE_TS_VERSION),
        );
    }
}

fn sample_replace_tsbuildinfo_version(sys: &mut TestSys) {
    if !sys.for_incremental_correctness {
        for file in [
            "/user/username/projects/sample1/core/tsconfig.tsbuildinfo",
            "/user/username/projects/sample1/logic/tsconfig.tsbuildinfo",
            "/user/username/projects/sample1/tests/tsconfig.tsbuildinfo",
        ] {
            sys.replace_file_text(
                file,
                &format!(r#""version":"{}""#, harnessutil::FAKE_TS_VERSION),
                r#""version":"FakeTsPreviousVersion""#,
            );
        }
    }
}

fn sample_write_empty_extended_config(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/sample1/tests/tsconfig.base.json",
        &stringtestutil::dedent(
            r#"
            {
                "compilerOptions": { }
            }"#,
        ),
    );
}

fn sample_logic_declaration_dir_change(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/logic/tsconfig.json",
        r#""declaration": true,"#,
        r#""declaration": true,
        "declarationDir": "decls","#,
    );
}

fn sample_core_add_declaration_option(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/tsconfig.json",
        r#""incremental": true,"#,
        r#""incremental": true, "declaration": true,"#,
    );
}

fn sample_core_target_es5(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/tsconfig.json",
        "esnext",
        "es5",
    );
}

fn sample_core_module_nodenext(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/core/tsconfig.json",
        "node18",
        "nodenext",
    );
}

fn sample_tests_es_module_interop_true(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/tests/tsconfig.json",
        r#""esModuleInterop": false"#,
        r#""esModuleInterop": true"#,
    );
}

fn sample_write_logic_config(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/sample1/logic/tsconfig.json",
        &build_sample_logic_config(),
    );
}

fn sample_append_logic_error(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/sample1/logic/index.ts",
        "\nlet y: string = 10;",
    );
}

fn sample_append_core_error(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/sample1/core/index.ts",
        "\nlet x: string = 10;",
    );
}

fn sample_fix_logic_error(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/logic/index.ts",
        "\nlet y: string = 10;",
        "",
    );
}

fn sample_append_logic_local_function(sys: &mut TestSys) {
    sys.append_file(
        "/user/username/projects/sample1/logic/index.ts",
        "\nfunction someFn() { }",
    );
}

fn sample_export_logic_function(sys: &mut TestSys) {
    sys.replace_file_text(
        "/user/username/projects/sample1/logic/index.ts",
        "\nfunction someFn() { }",
        "\nexport function someFn() { }",
    );
}

fn sample_write_core_file3(sys: &mut TestSys) {
    sys.write_file_no_error(
        "/user/username/projects/sample1/core/file3.ts",
        "export const y = 10;",
    );
}

fn build_sample_incremental_error_test(sub_scenario: &str, options: Vec<&str>) -> TscInput {
    let mut args = vec!["-b", "-w", "tests"];
    args.extend(options.iter().copied());
    let expected_diff_with_logic_error = if options.contains(&"--stopBuildOnErrors") {
        stringtestutil::dedent(
            r#"
            Clean build will stop on error in core and will not report error in logic
            Watch build will retain previous errors from logic and report it
            "#,
        )
    } else {
        String::new()
    };
    tsc_input_with_cwd_edits(
        &format!("reportErrors {sub_scenario}"),
        build_sample_file_map(None),
        "/user/username/projects/sample1",
        args,
        vec![
            local_change("change logic", sample_append_logic_error),
            local_change_expected(
                "change core",
                sample_append_core_error,
                &expected_diff_with_logic_error,
            ),
            local_change("fix error in logic", sample_fix_logic_error),
        ],
    )
}

fn build_sample_stop_build_on_error_tests(options: Option<Vec<&str>>) -> Vec<TscInput> {
    let options_vec = options.unwrap_or_default();
    let mut args = vec!["--b", "tests", "--verbose", "--stopBuildOnErrors"];
    args.extend(options_vec.iter().copied());
    let mut edits = Vec::new();
    if options_vec.is_empty() {
        edits.extend(no_change_only_edit());
    }
    edits.push(local_change("fix error", sample_fix_core_error));
    let edits_without_core_ref = if options_vec.is_empty() {
        vec![
            no_change(),
            local_change("fix error", sample_fix_core_error),
        ]
    } else {
        vec![local_change("fix error", sample_fix_core_error)]
    };
    vec![
        tsc_input_with_cwd_edits(
            "skips builds downstream projects if upstream projects have errors with stopBuildOnErrors",
            build_sample_file_map(Some(build_sample_add_core_error)),
            "/user/username/projects/sample1",
            args.clone(),
            edits,
        ),
        tsc_input_with_cwd_edits(
            "skips builds downstream projects if upstream projects have errors with stopBuildOnErrors when test does not reference core",
            build_sample_file_map(Some(build_sample_tests_reference_logic_only_and_core_error)),
            "/user/username/projects/sample1",
            args,
            edits_without_core_ref,
        ),
    ]
}

fn build_transitive_references_file_map(modify: Option<fn(&mut FileMap)>) -> FileMap {
    let mut files = file_map(&[
        (
            "/user/username/projects/transitiveReferences/refs/a.d.ts",
            &stringtestutil::dedent(
                r#"
                    export class X {}
                    export class A {}
                "#,
            ),
        ),
        (
            "/user/username/projects/transitiveReferences/a.ts",
            &stringtestutil::dedent(
                r#"
                    export class A {}
                "#,
            ),
        ),
        (
            "/user/username/projects/transitiveReferences/b.ts",
            &stringtestutil::dedent(
                r#"
                    import {A} from '@ref/a';
                    export const b = new A();
                "#,
            ),
        ),
        (
            "/user/username/projects/transitiveReferences/c.ts",
            &stringtestutil::dedent(
                r#"
                    import {b} from './b';
                    import {X} from "@ref/a";
                    b;
                    X;
                "#,
            ),
        ),
        (
            "/user/username/projects/transitiveReferences/tsconfig.a.json",
            &stringtestutil::dedent(
                r#"
                {
                    "files": ["a.ts"],
                    "compilerOptions": {
                        "composite": true,
                    },
                }"#,
            ),
        ),
        (
            "/user/username/projects/transitiveReferences/tsconfig.b.json",
            &stringtestutil::dedent(
                r#"
                {
                    "files": ["b.ts"],
                    "compilerOptions": {
                        "composite": true,
                        "paths": {
                            "@ref/*": ["./*"],
                        },
                    },
                    "references": [{ "path": "tsconfig.a.json" }],
                }"#,
            ),
        ),
        (
            "/user/username/projects/transitiveReferences/tsconfig.c.json",
            &stringtestutil::dedent(
                r#"
                {
                    "files": ["c.ts"],
                    "compilerOptions": {
                        "paths": {
                            "@ref/*": ["./refs/*"],
                        },
                    },
                    "references": [{ "path": "tsconfig.b.json" }],
                }"#,
            ),
        ),
    ]);
    if let Some(modify) = modify {
        modify(&mut files);
    }
    files
}

fn transitive_references_external_module_name(files: &mut FileMap) {
    files.insert(
        "/user/username/projects/transitiveReferences/b.ts".to_owned(),
        "import {A} from 'a';\nexport const b = new A();".into_map_file(SystemTime::UNIX_EPOCH),
    );
    files.insert(
        "/user/username/projects/transitiveReferences/tsconfig.b.json".to_owned(),
        stringtestutil::dedent(
            r#"
            {
                "files": ["b.ts"],
                "compilerOptions": {
                    "composite": true,
                    "module": "nodenext",
                },
                "references": [{ "path": "tsconfig.a.json" }],
            }"#,
        )
        .into_map_file(SystemTime::UNIX_EPOCH),
    );
}

#[test]
fn test_build_command_line() {
    let mut t = RustTestingT;
    t.parallel();

    let mut test_cases = vec![
        tsc_input("help", FileMap::new(), vec!["--build", "--help"]),
        tsc_input(
            "locale",
            FileMap::new(),
            vec!["--build", "--help", "--locale", "en"],
        ),
        tsc_input(
            "bad locale",
            FileMap::new(),
            vec!["--build", "--help", "--locale", "whoops"],
        ),
        tsc_input_with_edits(
            "different options",
            build_command_line_different_options_map("composite"),
            vec!["--build", "--verbose"],
            vec![
                edit_command(
                    "with sourceMap",
                    vec!["--build", "--verbose", "--sourceMap"],
                ),
                TscEdit {
                    caption: "should re-emit only js so they dont contain sourcemap".to_owned(),
                    command_line_args: None,
                    edit: None,
                    expected_diff: String::new(),
                },
                edit_command(
                    "with declaration should not emit anything",
                    vec!["--build", "--verbose", "--declaration"],
                ),
                no_change(),
                edit_command(
                    "with declaration and declarationMap",
                    vec!["--build", "--verbose", "--declaration", "--declarationMap"],
                ),
                TscEdit {
                    caption: "should re-emit only dts so they dont contain sourcemap".to_owned(),
                    command_line_args: None,
                    edit: None,
                    expected_diff: String::new(),
                },
                edit_command(
                    "with emitDeclarationOnly should not emit anything",
                    vec!["--build", "--verbose", "--emitDeclarationOnly"],
                ),
                no_change(),
                local_change("local change", replace_a_local),
                edit_command(
                    "with declaration should not emit anything",
                    vec!["--build", "--verbose", "--declaration"],
                ),
                edit_command(
                    "with inlineSourceMap",
                    vec!["--build", "--verbose", "--inlineSourceMap"],
                ),
                edit_command(
                    "with sourceMap",
                    vec!["--build", "--verbose", "--sourceMap"],
                ),
            ],
        ),
        tsc_input_with_edits(
            "different options with incremental",
            build_command_line_different_options_map("incremental"),
            vec!["--build", "--verbose"],
            vec![
                edit_command(
                    "with sourceMap",
                    vec!["--build", "--verbose", "--sourceMap"],
                ),
                TscEdit {
                    caption: "should re-emit only js so they dont contain sourcemap".to_owned(),
                    command_line_args: None,
                    edit: None,
                    expected_diff: String::new(),
                },
                edit_command(
                    "with declaration, emit Dts and should not emit js",
                    vec!["--build", "--verbose", "--declaration"],
                ),
                edit_command(
                    "with declaration and declarationMap",
                    vec!["--build", "--verbose", "--declaration", "--declarationMap"],
                ),
                no_change(),
                local_change("local change", replace_a_local),
                edit_command(
                    "with declaration and declarationMap",
                    vec!["--build", "--verbose", "--declaration", "--declarationMap"],
                ),
                no_change(),
                edit_command(
                    "with inlineSourceMap",
                    vec!["--build", "--verbose", "--inlineSourceMap"],
                ),
                edit_command(
                    "with sourceMap",
                    vec!["--build", "--verbose", "--sourceMap"],
                ),
                TscEdit {
                    caption: "emit js files".to_owned(),
                    command_line_args: None,
                    edit: None,
                    expected_diff: String::new(),
                },
                edit_command(
                    "with declaration and declarationMap",
                    vec!["--build", "--verbose", "--declaration", "--declarationMap"],
                ),
                edit_command(
                    "with declaration and declarationMap, should not re-emit",
                    vec!["--build", "--verbose", "--declaration", "--declarationMap"],
                ),
            ],
        ),
    ];
    test_cases.extend(build_command_line_emit_declaration_only_test_cases(
        &["composite"],
        "",
    ));
    test_cases.extend(build_command_line_emit_declaration_only_test_cases(
        &["incremental", "declaration"],
        " with declaration and incremental",
    ));
    test_cases.extend(build_command_line_emit_declaration_only_test_cases(
        &["declaration"],
        " with declaration",
    ));

    for test_case in test_cases {
        test_case.run(&mut t, "commandLine");
    }
}

#[test]
fn test_build_clean() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd(
            "file name and output name clashing",
            file_map(&[
                ("/home/src/workspaces/solution/index.js", ""),
                ("/home/src/workspaces/solution/bar.ts", ""),
                (
                    "/home/src/workspaces/solution/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "allowJs": true }
                        }"#,
                    ),
                ),
            ]),
            "/home/src/workspaces/solution",
            vec!["--b", "--clean"],
        ),
        tsc_input_with_cwd_edits(
            "tsx with dts emit",
            file_map(&[
                (
                    "/home/src/workspaces/solution/project/src/main.tsx",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/solution/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "declaration": true },
                            "include": ["src/**/*.tsx", "src/**/*.ts"]
                        }"#,
                    ),
                ),
            ]),
            "/home/src/workspaces/solution",
            vec!["--b", "project", "-v", "--explainFiles"],
            vec![
                no_change(),
                edit_command("clean build", vec!["-b", "project", "--clean"]),
            ],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "clean");
    }
}

#[test]
fn test_build_config_file_errors() {
    let mut t = RustTestingT;
    t.parallel();

    let syntax_error_files = || {
        file_map(&[
            (
                "/home/src/workspaces/project/a.ts",
                "export function foo() { }",
            ),
            (
                "/home/src/workspaces/project/b.ts",
                "export function bar() { }",
            ),
            (
                "/home/src/workspaces/project/tsconfig.json",
                &stringtestutil::dedent(
                    r#"
                    {
                        "compilerOptions": {
                            "composite": true,
                        },
                        "files": [
                            "a.ts"
                            "b.ts"
                        ]
                    }"#,
                ),
            ),
        ])
    };

    let test_cases = vec![
        tsc_input(
            "when tsconfig extends the missing file",
            file_map(&[
                (
                    "/home/src/workspaces/project/tsconfig.first.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "extends": "./foobar.json",
                            "compilerOptions": {
                                "composite": true
                            }
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/tsconfig.second.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "extends": "./foobar.json",
                            "compilerOptions": {
                                "composite": true
                            }
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "composite": true
                            },
                            "references": [
                                { "path": "./tsconfig.first.json" },
                                { "path": "./tsconfig.second.json" }
                            ]
                        }"#,
                    ),
                ),
            ]),
            vec!["--b"],
        ),
        tsc_input_with_edits(
            "reports syntax errors in config file",
            syntax_error_files(),
            vec!["--b"],
            vec![
                local_change(
                    "reports syntax errors after change to config file",
                    replace_config_comma_with_declaration,
                ),
                local_change(
                    "reports syntax errors after change to ts file",
                    append_foo_bar_to_a,
                ),
                no_change(),
                local_change("builds after fixing config file errors", write_fixed_config),
            ],
        ),
        tsc_input(
            "missing config file",
            FileMap::new(),
            vec!["--b", "bogus.json"],
        ),
        tsc_input_with_edits(
            "reports syntax errors in config file",
            syntax_error_files(),
            vec!["--b", "-w"],
            vec![
                local_change(
                    "reports syntax errors after change to config file",
                    replace_config_comma_with_declaration,
                ),
                local_change(
                    "reports syntax errors after change to ts file",
                    append_foo_bar_to_a,
                ),
                local_change(
                    "reports error when there is no change to tsconfig file",
                    touch_config_no_text_change,
                ),
                local_change("builds after fixing config file errors", write_fixed_config),
            ],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "configFileErrors");
    }
}

#[test]
fn test_build_demo_project() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd_edits(
            "in master branch with everything setup correctly and reports no error",
            build_demo_file_map(None),
            "/user/username/projects/demo",
            vec!["--b", "--verbose"],
            no_change_only_edit(),
        ),
        tsc_input_with_cwd(
            "in circular branch reports the error about it by stopping build",
            build_demo_file_map(Some(demo_with_core_ref_to_zoo)),
            "/user/username/projects/demo",
            vec!["--b", "--verbose"],
        ),
        tsc_input_with_cwd(
            "in bad-ref branch reports the error about files not in rootDir at the import location",
            build_demo_file_map(Some(demo_with_bad_ref)),
            "/user/username/projects/demo",
            vec!["--b", "--verbose"],
        ),
        tsc_input_with_cwd(
            "in circular is set in the reference",
            build_demo_file_map(Some(demo_with_circular_reference_option)),
            "/user/username/projects/demo",
            vec!["--b", "--verbose"],
        ),
        tsc_input_with_cwd_edits(
            "updates with circular reference",
            build_demo_file_map(Some(demo_with_core_ref_to_zoo)),
            "/user/username/projects/demo",
            vec!["--b", "-w", "--verbose"],
            vec![local_change("Fix error", fix_demo_core_config)],
        ),
        tsc_input_with_cwd_edits(
            "updates with bad reference",
            build_demo_file_map(Some(demo_with_bad_ref)),
            "/user/username/projects/demo",
            vec!["--b", "-w", "--verbose"],
            vec![local_change(
                "Prepend a line",
                prepend_blank_to_demo_core_utilities,
            )],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "demo");
    }
}

#[test]
fn test_build_emit_declaration_only() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        emit_declaration_only_test_case(false),
        emit_declaration_only_test_case(true),
        tsc_input_with_edits(
            "only dts output in non circular imports project with emitDeclarationOnly",
            build_emit_declaration_only_import_file_map(true, false),
            vec!["--b", "--verbose"],
            vec![
                local_change(
                    "incremental-declaration-doesnt-change",
                    add_class_c_to_emit_declaration_only_a,
                ),
                local_change(
                    "incremental-declaration-changes",
                    add_foo_to_emit_declaration_only_a,
                ),
            ],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "emitDeclarationOnly");
    }
}

#[test]
fn test_build_file_delete() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd_edits(
            "detects deleted file",
            file_map(&[
                (
                    "/home/src/workspaces/solution/child/child.ts",
                    &stringtestutil::dedent(
                        r#"
                        import { child2 } from "../child/child2";
                        export function child() {
                            child2();
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/solution/child/child2.ts",
                    &stringtestutil::dedent(
                        r#"
                        export function child2() {
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/solution/child/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "composite": true }
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/solution/main/main.ts",
                    &stringtestutil::dedent(
                        r#"
                        import { child } from "../child/child";
                        export function main() {
                            child();
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/solution/main/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "composite": true },
                            "references": [{ "path": "../child" }],
                        }"#,
                    ),
                ),
            ]),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "main/tsconfig.json",
                "-v",
                "--traceResolution",
                "--explainFiles",
            ],
            vec![local_change(
                "delete child2 file",
                remove_child2_composite_outputs,
            )],
        ),
        tsc_input_with_cwd_edits(
            "deleted file without composite",
            file_map(&[
                (
                    "/home/src/workspaces/solution/child/child.ts",
                    &stringtestutil::dedent(
                        r#"
                        import { child2 } from "../child/child2";
                        export function child() {
                            child2();
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/solution/child/child2.ts",
                    &stringtestutil::dedent(
                        r#"
                        export function child2() {
                        }"#,
                    ),
                ),
                (
                    "/home/src/workspaces/solution/child/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { }
                        }"#,
                    ),
                ),
            ]),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "child/tsconfig.json",
                "-v",
                "--traceResolution",
                "--explainFiles",
            ],
            vec![local_change(
                "delete child2 file",
                remove_child2_non_composite_outputs,
            )],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "fileDelete");
    }
}

#[test]
fn test_build_inferred_type_from_transitive_module() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_edits(
            "inferred type from transitive module",
            build_inferred_type_from_transitive_module_map(false, ""),
            vec!["--b", "--verbose"],
            vec![
                local_change("incremental-declaration-changes", remove_bar_param_type),
                local_change("incremental-declaration-changes", restore_bar_param_type),
            ],
        ),
        tsc_input_with_edits(
            "inferred type from transitive module with isolatedModules",
            build_inferred_type_from_transitive_module_map(true, ""),
            vec!["--b", "--verbose"],
            vec![
                local_change("incremental-declaration-changes", remove_bar_param_type),
                local_change("incremental-declaration-changes", restore_bar_param_type),
            ],
        ),
        tsc_input_with_edits(
            "reports errors in files affected by change in signature with isolatedModules",
            build_inferred_type_from_transitive_module_map(
                true,
                &stringtestutil::dedent(
                    r#"
                    import { default as bar } from './bar';
                    bar("hello");"#,
                ),
            ),
            vec!["--b", "--verbose"],
            vec![
                local_change("incremental-declaration-changes", remove_bar_param_type),
                local_change("incremental-declaration-changes", restore_bar_param_type),
                local_change("incremental-declaration-changes", remove_bar_param_type),
                local_change("Fix Error", fix_lazy_index_bar_call),
            ],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "inferredTypeFromTransitiveModule");
    }
}

#[test]
fn test_build_inferred_type_from_monorepo_reference() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![tsc_input_with_cwd(
        "inferred type from referenced project that references another project in monorepo",
        build_inferred_type_from_monorepo_reference_map(),
        "/home/src/workspaces/solution",
        vec!["--b", "--verbose"],
    )];

    for test_case in test_cases {
        test_case.run(&mut t, "inferredTypeFromMonorepoReference");
    }
}

#[test]
fn test_build_javascript_project_emit() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd(
            "loads js-based projects and emits them correctly",
            build_javascript_project_emit_map(),
            "/home/src/workspaces/solution",
            vec!["--b"],
        ),
        tsc_input_with_cwd(
            "loads js-based projects with non-moved json files and emits them correctly",
            build_javascript_project_emit_non_moved_json_map(),
            "/home/src/workspaces/solution",
            vec!["-b"],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "javascriptProjectEmit");
    }
}

#[test]
fn test_build_late_bound_symbol() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![tsc_input_with_edits(
        "interface is merged and contains late bound member",
        build_late_bound_symbol_map(),
        vec!["--b", "--verbose"],
        vec![
            local_change(
                "incremental-declaration-doesnt-change",
                remove_late_bound_symbol_unrelated_const,
            ),
            local_change(
                "incremental-declaration-doesnt-change",
                append_late_bound_symbol_unrelated_const,
            ),
        ],
    )];

    for test_case in test_cases {
        test_case.run(&mut t, "lateBoundSymbol");
    }
}

#[test]
fn test_build_module_specifiers() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd(
            "synthesized module specifiers resolve correctly",
            build_module_specifiers_synthesized_resolve_map(),
            "/home/src/workspaces/packages",
            vec!["-b", "--verbose"],
        ),
        tsc_input_with_cwd(
            "synthesized module specifiers across projects resolve correctly",
            build_module_specifiers_across_projects_map(),
            "/home/src/workspaces/packages",
            vec!["-b", "src-types", "src-dogs", "--verbose"],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "moduleSpecifiers");
    }
}

#[test]
fn test_build_output_paths() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_output_path_scenario(
            "when rootDir is not specified",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "outDir": "dist",
                            },
                        }"#,
                    ),
                ),
            ]),
            vec!["/home/src/workspaces/project/dist/src/index.js"],
        ),
        tsc_output_path_scenario(
            "when rootDir is not specified and is composite",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "outDir": "dist",
                                "composite": true,
                            },
                        }"#,
                    ),
                ),
            ]),
            vec![
                "/home/src/workspaces/project/dist/src/index.js",
                "/home/src/workspaces/project/dist/src/index.d.ts",
            ],
        ),
        tsc_output_path_scenario(
            "when rootDir is specified",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "outDir": "dist",
                                "rootDir": "src",
                            },
                        }"#,
                    ),
                ),
            ]),
            vec!["/home/src/workspaces/project/dist/index.js"],
        ),
        tsc_output_path_scenario(
            "when rootDir is specified but not all files belong to rootDir",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/types/type.ts",
                    "export type t = string;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "outDir": "dist",
                                "rootDir": "src",
                            },
                        }"#,
                    ),
                ),
            ]),
            vec![
                "/home/src/workspaces/project/dist/index.js",
                "/home/src/workspaces/project/types/type.js",
            ],
        ),
        tsc_output_path_scenario(
            "when rootDir is specified but not all files belong to rootDir and is composite",
            file_map(&[
                (
                    "/home/src/workspaces/project/src/index.ts",
                    "export const x = 10;",
                ),
                (
                    "/home/src/workspaces/project/types/type.ts",
                    "export type t = string;",
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "outDir": "dist",
                                "rootDir": "src",
                                "composite": true
                            },
                        }"#,
                    ),
                ),
            ]),
            vec![
                "/home/src/workspaces/project/dist/index.js",
                "/home/src/workspaces/project/dist/index.d.ts",
                "/home/src/workspaces/project/types/type.js",
                "/home/src/workspaces/project/types/type.d.ts",
            ],
        ),
    ];

    for test_case in test_cases {
        run_output_paths(&mut t, test_case);
    }
}

#[test]
fn test_build_program_updates() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd_edits(
            "when referenced project change introduces error in the down stream project and then fixes it",
            program_updates_referenced_project_error_map(),
            "/user/username/projects/sample1",
            vec!["-b", "-w", "App"],
            vec![
                local_change("Introduce error", program_updates_message_to_message2),
                local_change("Fix error", program_updates_message2_to_message),
            ],
        ),
        tsc_input_with_cwd_edits(
            "declarationEmitErrors when fixing error files all files are emitted",
            program_updates_declaration_emit_errors_map(true),
            "/user/username/projects/solution",
            vec!["-b", "-w", "app"],
            vec![local_change(
                "Fix error",
                program_updates_fix_file_with_error,
            )],
        ),
        tsc_input_with_cwd_edits(
            "declarationEmitErrors when file with no error changes",
            program_updates_declaration_emit_errors_map(true),
            "/user/username/projects/solution",
            vec!["-b", "-w", "app"],
            vec![local_change(
                "Change fileWithoutError",
                program_updates_change_file_without_error,
            )],
        ),
        tsc_input_with_cwd_edits(
            "declarationEmitErrors introduceError when fixing errors only changed file is emitted",
            program_updates_declaration_emit_errors_map(false),
            "/user/username/projects/solution",
            vec!["-b", "-w", "app"],
            vec![
                local_change("Introduce error", program_updates_introduce_file_with_error),
                local_change("Fix error", program_updates_fix_file_with_error),
            ],
        ),
        tsc_input_with_cwd_edits(
            "declarationEmitErrors introduceError when file with no error changes",
            program_updates_declaration_emit_errors_map(false),
            "/user/username/projects/solution",
            vec!["-b", "-w", "app"],
            vec![
                local_change("Introduce error", program_updates_introduce_file_with_error),
                local_change(
                    "Change fileWithoutError",
                    program_updates_change_file_without_error,
                ),
            ],
        ),
        tsc_input_with_cwd_edits(
            "works when noUnusedParameters changes to false",
            file_map(&[
                (
                    "/user/username/projects/myproject/index.ts",
                    "const fn = (a: string, b: string) => b;",
                ),
                (
                    "/user/username/projects/myproject/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": {
                                "noUnusedParameters": true,
                            },
                        }"#,
                    ),
                ),
            ]),
            "/user/username/projects/myproject",
            vec!["-b", "-w"],
            vec![local_change(
                "Change tsconfig to set noUnusedParameters to false",
                program_updates_set_no_unused_parameters_false,
            )],
        ),
        tsc_input_with_cwd_edits(
            "works with extended source files",
            program_updates_extended_source_files_map(),
            "/user/username/projects/project",
            vec![
                "-b",
                "-w",
                "-v",
                "project1.tsconfig.json",
                "project2.tsconfig.json",
                "project3.tsconfig.json",
            ],
            vec![
                local_change("Modify alpha config", program_updates_modify_alpha_config),
                local_change("change bravo config", program_updates_change_bravo_config),
                local_change(
                    "project 2 extends alpha",
                    program_updates_project2_extends_alpha,
                ),
                local_change("update aplha config", program_updates_alpha_config_empty),
                local_change(
                    "Modify extendsConfigFile2",
                    program_updates_modify_extends_config_file2,
                ),
                local_change("Modify project 3", program_updates_modify_project3),
                local_change(
                    "Delete extendedConfigFile2 and report error",
                    program_updates_delete_extends_config_file2,
                ),
            ],
        ),
        tsc_input_with_cwd_edits(
            "works correctly when project with extended config is removed",
            program_updates_project_with_extended_config_removed_map(),
            "/user/username/projects/project",
            vec!["-b", "-w", "-v"],
            vec![local_change(
                "Remove project2 from base config",
                program_updates_remove_project2_from_base_config,
            )],
        ),
        tsc_input_with_cwd(
            "tsbuildinfo has error",
            file_map(&[
                (
                    "/user/username/projects/project/main.ts",
                    "export const x = 10;",
                ),
                ("/user/username/projects/project/tsconfig.json", "{}"),
                (
                    "/user/username/projects/project/tsconfig.tsbuildinfo",
                    "Some random string",
                ),
            ]),
            "/user/username/projects/project",
            vec!["--b", "-i", "-w"],
        ),
        tsc_input_with_cwd_edits(
            "when root is source from project reference",
            program_updates_root_source_from_project_reference_map(false),
            "/home/src/workspaces/project",
            vec!["--b"],
            vec![local_change(
                "dts doesnt change",
                program_updates_append_bar_to_lib_foo,
            )],
        ),
        tsc_input_with_cwd_edits(
            "when root is source from project reference with composite",
            program_updates_root_source_from_project_reference_map(true),
            "/home/src/workspaces/project",
            vec!["--b"],
            vec![local_change(
                "dts doesnt change",
                program_updates_append_bar_to_lib_foo,
            )],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "programUpdates");
    }
}

#[test]
fn test_build_projects_building() {
    let mut t = RustTestingT;
    t.parallel();

    let mut test_cases = Vec::new();
    test_cases.extend(projects_building_test_cases(3, 1));
    test_cases.extend(projects_building_test_cases(5, 2));
    test_cases.extend(projects_building_test_cases(8, 3));
    test_cases.extend(projects_building_test_cases(23, 3));

    for test_case in test_cases {
        test_case.run(&mut t, "projectsBuilding");
    }
}

#[test]
fn test_build_project_reference_with_root_dir_in_parent() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd(
            "builds correctly",
            build_project_reference_with_root_dir_in_parent_file_map(
                ProjectReferenceWithRootDirInParentVariant::Default,
            ),
            "/home/src/workspaces/solution",
            vec!["--b", "src/main", "/home/src/workspaces/solution/src/other"],
        ),
        tsc_input_with_cwd(
            "reports error for same tsbuildinfo file because no rootDir in the base",
            build_project_reference_with_root_dir_in_parent_file_map(
                ProjectReferenceWithRootDirInParentVariant::NoRootDirInBase,
            ),
            "/home/src/workspaces/solution",
            vec!["--b", "src/main", "--verbose"],
        ),
        tsc_input_with_cwd_edits(
            "reports error for same tsbuildinfo file",
            build_project_reference_with_root_dir_in_parent_file_map(
                ProjectReferenceWithRootDirInParentVariant::SameTsBuildInfo,
            ),
            "/home/src/workspaces/solution",
            vec!["--b", "src/main", "--verbose"],
            no_change_only_edit(),
        ),
        tsc_input_with_cwd(
            "reports error for same tsbuildinfo file without incremental",
            build_project_reference_with_root_dir_in_parent_file_map(
                ProjectReferenceWithRootDirInParentVariant::SameTsBuildInfoWithoutIncremental,
            ),
            "/home/src/workspaces/solution",
            vec!["--b", "src/main", "--verbose"],
        ),
        tsc_input_with_cwd_edits(
            "reports error for same tsbuildinfo file without incremental with tsc",
            build_project_reference_with_root_dir_in_parent_file_map(
                ProjectReferenceWithRootDirInParentVariant::SameTsBuildInfoWithoutIncremental,
            ),
            "/home/src/workspaces/solution",
            vec!["--b", "src/other", "--verbose"],
            vec![edit_command("Running tsc on main", vec!["-p", "src/main"])],
        ),
        tsc_input_with_cwd_edits(
            "reports no error when tsbuildinfo differ",
            build_project_reference_with_root_dir_in_parent_file_map(
                ProjectReferenceWithRootDirInParentVariant::TsBuildInfoDiffer,
            ),
            "/home/src/workspaces/solution",
            vec!["--b", "src/main/tsconfig.main.json", "--verbose"],
            no_change_only_edit(),
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "projectReferenceWithRootDirInParent");
    }
}

#[test]
fn test_build_reexport() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![tsc_input_with_cwd_edits(
        "Reports errors correctly",
        build_reexport_file_map(),
        "/user/username/projects/reexport",
        vec!["-b", "-w", "-verbose", "src"],
        vec![
            local_change("Introduce error", introduce_reexport_session_error),
            local_change("Fix error", fix_reexport_session_error),
        ],
    )];

    for test_case in test_cases {
        test_case.run(&mut t, "reexport");
    }
}

#[test]
fn test_build_resolve_json_module() {
    let mut t = RustTestingT;
    t.parallel();

    let scenarios = build_resolve_json_module_scenarios();
    let mut test_cases = build_resolve_json_module_test_cases(&scenarios);
    test_cases.push(tsc_input_with_cwd_edits(
        "importing json module from project reference",
        build_resolve_json_module_project_reference_file_map(),
        "/home/src/workspaces/solution",
        vec!["--b", "project", "--verbose", "--explainFiles"],
        no_change_only_edit(),
    ));

    for test_case in test_cases {
        test_case.run(&mut t, "resolveJsonModule");
    }
}

#[test]
fn test_build_roots() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_edits(
            "when two root files are consecutive",
            file_map(&[
                (
                    "/home/src/workspaces/project/file1.ts",
                    r#"export const x = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/file2.ts",
                    r#"export const y = "world";"#,
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "composite": true },
                            "include": ["*.ts"],
                        }"#,
                    ),
                ),
            ]),
            vec!["--b", "-v"],
            vec![local_change("delete file1", delete_project_file1_outputs)],
        ),
        tsc_input_with_edits(
            "when multiple root files are consecutive",
            file_map(&[
                (
                    "/home/src/workspaces/project/file1.ts",
                    r#"export const x = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/file2.ts",
                    r#"export const y = "world";"#,
                ),
                (
                    "/home/src/workspaces/project/file3.ts",
                    r#"export const y = "world";"#,
                ),
                (
                    "/home/src/workspaces/project/file4.ts",
                    r#"export const y = "world";"#,
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "composite": true },
                            "include": ["*.ts"],
                        }"#,
                    ),
                ),
            ]),
            vec!["--b", "-v"],
            vec![local_change("delete file1", delete_project_file1_outputs)],
        ),
        tsc_input_with_edits(
            "when files are not consecutive",
            file_map(&[
                (
                    "/home/src/workspaces/project/file1.ts",
                    r#"export const x = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/random.d.ts",
                    r#"export const random = "world";"#,
                ),
                (
                    "/home/src/workspaces/project/file2.ts",
                    &stringtestutil::dedent(
                        r#"
                        import { random } from "./random";
                        export const y = "world";
                    "#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "composite": true },
                            "include": ["file*.ts"],
                        }"#,
                    ),
                ),
            ]),
            vec!["--b", "-v"],
            vec![local_change("delete file1", delete_project_file1_outputs)],
        ),
        tsc_input_with_edits(
            "when consecutive and non consecutive are mixed",
            file_map(&[
                (
                    "/home/src/workspaces/project/file1.ts",
                    r#"export const x = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/file2.ts",
                    r#"export const y = "world";"#,
                ),
                (
                    "/home/src/workspaces/project/random.d.ts",
                    r#"export const random = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/nonconsecutive.ts",
                    &stringtestutil::dedent(
                        r#"
                        import { random } from "./random";
                            export const nonConsecutive = "hello";
                    "#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/random1.d.ts",
                    r#"export const random = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/asArray1.ts",
                    &stringtestutil::dedent(
                        r#"
                        import { random } from "./random1";
                        export const x = "hello";
                    "#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/asArray2.ts",
                    r#"export const x = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/asArray3.ts",
                    r#"export const x = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/random2.d.ts",
                    r#"export const random = "hello";"#,
                ),
                (
                    "/home/src/workspaces/project/anotherNonConsecutive.ts",
                    &stringtestutil::dedent(
                        r#"
                        import { random } from "./random2";
                        export const nonConsecutive = "hello";
                    "#,
                    ),
                ),
                (
                    "/home/src/workspaces/project/tsconfig.json",
                    &stringtestutil::dedent(
                        r#"
                        {
                            "compilerOptions": { "composite": true },
                            "include": ["file*.ts", "nonconsecutive*.ts", "asArray*.ts", "anotherNonConsecutive.ts"],
                        }"#,
                    ),
                ),
            ]),
            vec!["--b", "-v"],
            vec![local_change("delete file1", delete_project_file1_outputs)],
        ),
        tsc_input_with_cwd_edits(
            "when root file is from referenced project",
            build_roots_from_project_referenced_project_file_map(true),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "projects/server",
                "-v",
                "--traceResolution",
                "--explainFiles",
            ],
            build_roots_from_project_referenced_project_test_edits(),
        ),
        tsc_input_with_cwd_edits(
            "when root file is from referenced project and shared is first",
            build_roots_from_project_referenced_project_file_map(false),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "projects/server",
                "-v",
                "--traceResolution",
                "--explainFiles",
            ],
            build_roots_from_project_referenced_project_test_edits(),
        ),
        tsc_input_with_cwd_edits(
            "when root file is from referenced project",
            build_roots_from_project_referenced_project_file_map(true),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "-w",
                "projects/server",
                "-v",
                "--traceResolution",
                "--explainFiles",
            ],
            build_roots_from_project_referenced_project_test_edits(),
        ),
        tsc_input_with_cwd_edits(
            "when root file is from referenced project and shared is first",
            build_roots_from_project_referenced_project_file_map(false),
            "/home/src/workspaces/solution",
            vec![
                "--b",
                "-w",
                "projects/server",
                "-v",
                "--traceResolution",
                "--explainFiles",
            ],
            build_roots_from_project_referenced_project_test_edits(),
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "roots");
    }
}

#[test]
fn test_build_sample() {
    let mut t = RustTestingT;
    t.parallel();

    let mut sample_edits = build_sample_core_change_edits();
    sample_edits.push(local_change(
        "when logic config changes declaration dir",
        sample_logic_declaration_dir_change,
    ));
    sample_edits.push(no_change());

    let mut test_cases = vec![
        tsc_input_with_cwd(
            "builds correctly when outDir is specified",
            build_sample_file_map(Some(build_sample_logic_config_out_dir)),
            "/user/username/projects/sample1",
            vec!["--b", "tests"],
        ),
        tsc_input_with_cwd(
            "builds correctly when declarationDir is specified",
            build_sample_file_map(Some(build_sample_logic_config_declaration_dir)),
            "/user/username/projects/sample1",
            vec!["--b", "tests"],
        ),
        tsc_input_with_cwd(
            "builds correctly when project is not composite or doesnt have any references",
            build_sample_file_map(Some(build_sample_core_not_composite)),
            "/user/username/projects/sample1",
            vec!["--b", "core", "--verbose"],
        ),
        tsc_input_with_cwd(
            "does not write any files in a dry build",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--dry"],
        ),
        tsc_input_with_cwd_edits(
            "removes all files it built",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests"],
            vec![
                edit_command(
                    "removes all files it built",
                    vec!["--b", "tests", "--clean"],
                ),
                edit_command("no change --clean", vec!["--b", "tests", "--clean"]),
            ],
        ),
        tsc_input_with_cwd(
            "cleaning project in not build order doesnt throw error",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "logic2", "--clean"],
        ),
        tsc_input_with_cwd_edits(
            "always builds under with force option",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--force"],
            no_change_only_edit(),
        ),
        tsc_input_with_cwd_edits(
            "can detect when and what to rebuild",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            vec![
                no_change(),
                local_change(
                    "Only builds the leaf node project",
                    sample_write_tests_index_const,
                ),
                local_change(
                    "Detects type-only changes in upstream projects",
                    sample_replace_core_hello_world,
                ),
                local_change(
                    "rebuilds when tsconfig changes",
                    sample_rebuild_tests_target_es2020,
                ),
            ],
        ),
        tsc_input_with_cwd_edits(
            "when input file text does not change but its modified time changes",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            vec![local_change(
                "upstream project changes without changing file text",
                sample_touch_core_index,
            )],
        ),
        tsc_input_with_cwd_edits(
            "when declarationMap changes",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            vec![
                local_change("Disable declarationMap", sample_disable_declaration_map),
                local_change("Enable declarationMap", sample_enable_declaration_map),
            ],
        ),
        tsc_input_with_cwd_edits(
            "indicates that it would skip builds during a dry build",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests"],
            vec![edit_command("--dry", vec!["--b", "tests", "--dry"])],
        ),
        tsc_input_with_cwd_edits(
            "rebuilds from start if force option is set",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests"],
            vec![edit_command(
                "--force build",
                vec!["--b", "tests", "--verbose", "--force"],
            )],
        ),
        tsc_input_with_edits(
            "tsbuildinfo has error",
            file_map(&[
                (
                    "/home/src/workspaces/project/main.ts",
                    "export const x = 10;",
                ),
                ("/home/src/workspaces/project/tsconfig.json", "{}"),
                (
                    "/home/src/workspaces/project/tsconfig.tsbuildinfo",
                    "Some random string",
                ),
            ]),
            vec!["--b", "-i", "-v"],
            vec![local_change(
                "tsbuildinfo written has error",
                sample_prepend_bad_tsbuildinfo,
            )],
        ),
        tsc_input_with_cwd_edits(
            "rebuilds completely when version in tsbuildinfo doesnt match ts version",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            vec![local_change(
                "convert tsbuildInfo version to something that is say to previous version",
                sample_replace_tsbuildinfo_version,
            )],
        ),
        tsc_input_with_cwd_edits(
            "rebuilds when extended config file changes",
            build_sample_file_map(Some(build_sample_extended_config)),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            vec![local_change(
                "change extended file",
                sample_write_empty_extended_config,
            )],
        ),
        tsc_input_with_cwd(
            "building project in not build order doesnt throw error",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "logic2/tsconfig.json", "--verbose"],
        ),
        tsc_input_with_cwd_edits(
            "builds downstream projects even if upstream projects have errors",
            build_sample_file_map(Some(build_sample_logic_error)),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            no_change_only_edit(),
        ),
        tsc_input_with_cwd_edits(
            "listFiles",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--listFiles"],
            build_sample_core_change_edits(),
        ),
        tsc_input_with_cwd_edits(
            "listEmittedFiles",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--listEmittedFiles"],
            build_sample_core_change_edits(),
        ),
        tsc_input_with_cwd_edits(
            "explainFiles",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--explainFiles", "--v"],
            build_sample_core_change_edits(),
        ),
        tsc_input_with_cwd_edits(
            "sample",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            sample_edits,
        ),
        tsc_input_with_cwd(
            "when logic specifies tsBuildInfoFile",
            build_sample_file_map(Some(build_sample_logic_tsbuildinfo_file)),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
        ),
        tsc_input_with_cwd_edits(
            "when declaration option changes",
            build_sample_file_map(Some(build_sample_core_incremental_no_declaration)),
            "/user/username/projects/sample1",
            vec!["--b", "core", "--verbose"],
            vec![local_change(
                "incremental-declaration-changes",
                sample_core_add_declaration_option,
            )],
        ),
        tsc_input_with_cwd_edits(
            "when target option changes",
            build_sample_file_map(Some(build_sample_core_target_esnext)),
            "/user/username/projects/sample1",
            vec!["--b", "core", "--verbose"],
            vec![local_change(
                "incremental-declaration-changes",
                sample_core_target_es5,
            )],
        ),
        tsc_input_with_cwd_edits(
            "when module option changes",
            build_sample_file_map(Some(build_sample_core_module_node18)),
            "/user/username/projects/sample1",
            vec!["--b", "core", "--verbose"],
            vec![local_change(
                "incremental-declaration-changes",
                sample_core_module_nodenext,
            )],
        ),
        tsc_input_with_cwd_edits(
            "when esModuleInterop option changes",
            build_sample_file_map(Some(build_sample_tests_es_module_interop_false)),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
            vec![local_change(
                "incremental-declaration-changes",
                sample_tests_es_module_interop_true,
            )],
        ),
        tsc_input_with_cwd(
            "reports error if input file is missing",
            build_sample_file_map(Some(build_sample_missing_input_file)),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose"],
        ),
        tsc_input_with_cwd(
            "reports error if input file is missing with force",
            build_sample_file_map(Some(build_sample_missing_input_file)),
            "/user/username/projects/sample1",
            vec!["--b", "tests", "--verbose", "--force"],
        ),
        tsc_input_with_cwd_edits(
            "change builds changes and reports found errors message",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests"],
            build_sample_watch_dts_changing_edits(),
        ),
        tsc_input_with_cwd_edits(
            "non local change does not start build of referencing projects",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests"],
            build_sample_watch_non_dts_changing_edits(),
        ),
        tsc_input_with_cwd_edits(
            "builds when new file is added, and its subsequent updates",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests"],
            build_sample_watch_new_file_edits(),
        ),
        tsc_input_with_cwd_edits(
            "change builds changes and reports found errors message with circular references",
            build_sample_file_map(Some(build_sample_make_circular_references)),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests"],
            build_sample_watch_dts_changing_edits(),
        ),
        tsc_input_with_cwd_edits(
            "non local change does not start build of referencing projects with circular references",
            build_sample_file_map(Some(build_sample_make_circular_references)),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests"],
            build_sample_watch_non_dts_changing_edits(),
        ),
        tsc_input_with_cwd_edits(
            "builds when new file is added, and its subsequent updates with circular references",
            build_sample_file_map(Some(build_sample_make_circular_references)),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests"],
            build_sample_watch_new_file_edits(),
        ),
        tsc_input_with_cwd_edits(
            "watches config files that are not present",
            build_sample_file_map(Some(build_sample_delete_logic_config)),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests"],
            vec![local_change("Write logic", sample_write_logic_config)],
        ),
        build_sample_incremental_error_test("when preserveWatchOutput is not used", Vec::new()),
        build_sample_incremental_error_test(
            "when preserveWatchOutput is passed on command line",
            vec!["--preserveWatchOutput"],
        ),
        build_sample_incremental_error_test(
            "when stopBuildOnErrors is passed on command line",
            vec!["--stopBuildOnErrors"],
        ),
        tsc_input_with_cwd_edits(
            "incremental updates in verbose mode",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "tests", "--verbose"],
            vec![
                local_change("Make non dts change", sample_append_logic_local_function),
                local_change("Make dts change", sample_export_logic_function),
            ],
        ),
        tsc_input_with_cwd_edits(
            "should not trigger recompilation because of program emit",
            build_sample_file_map(None),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "core", "--verbose"],
            vec![
                no_change(),
                local_change("Add new file", sample_write_core_file3),
                no_change(),
            ],
        ),
        tsc_input_with_cwd_edits(
            "should not trigger recompilation because of program emit with outDir specified",
            build_sample_file_map(Some(build_sample_core_out_dir)),
            "/user/username/projects/sample1",
            vec!["--b", "-w", "core", "--verbose"],
            vec![
                no_change(),
                local_change("Add new file", sample_write_core_file3),
                no_change(),
            ],
        ),
    ];
    test_cases.extend(build_sample_stop_build_on_error_tests(None));
    test_cases.extend(build_sample_stop_build_on_error_tests(Some(vec![
        "--watch",
    ])));

    for test_case in test_cases {
        test_case.run(&mut t, "sample");
    }
}

#[test]
fn test_build_transitive_references() {
    let mut t = RustTestingT;
    t.parallel();

    let test_cases = vec![
        tsc_input_with_cwd(
            "builds correctly",
            build_transitive_references_file_map(None),
            "/user/username/projects/transitiveReferences",
            vec!["--b", "tsconfig.c.json", "--listFiles"],
        ),
        tsc_input_with_cwd(
            "reports error about module not found with node resolution with external module name",
            build_transitive_references_file_map(Some(transitive_references_external_module_name)),
            "/user/username/projects/transitiveReferences",
            vec!["--b", "tsconfig.c.json", "--listFiles"],
        ),
    ];

    for test_case in test_cases {
        test_case.run(&mut t, "transitiveReferences");
    }
}
