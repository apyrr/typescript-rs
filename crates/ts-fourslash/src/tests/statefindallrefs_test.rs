use crate::{new_fourslash, TestingT};
pub fn test_find_all_refs_solution_referencing_default_project_directly(t: &mut TestingT) {
    run_default_project_solution_scenario(t, false, false, false, false, false);
}

pub fn test_find_all_refs_solution_referencing_default_project_indirectly(t: &mut TestingT) {
    run_default_project_solution_scenario(t, true, false, false, false, false);
}

pub fn test_find_all_refs_solution_with_disable_referenced_project_load_referencing_default_project_directly(
    t: &mut TestingT,
) {
    run_default_project_solution_scenario(t, false, true, false, false, false);
}

pub fn test_find_all_refs_solution_referencing_default_project_indirectly_through_disable_referenced_project_load(
    t: &mut TestingT,
) {
    run_default_project_solution_scenario(t, true, false, true, true, false);
}

pub fn test_find_all_refs_solution_referencing_default_project_indirectly_through_disable_referenced_project_load_in_one_but_without_it_in_another(
    t: &mut TestingT,
) {
    run_default_project_solution_scenario(t, true, false, true, false, false);
}

pub fn test_find_all_refs_project_with_own_files_referencing_file_from_referenced_project(
    t: &mut TestingT,
) {
    run_default_project_solution_scenario(t, false, false, false, false, true);
}

pub fn test_find_all_refs_root_of_referenced_project(t: &mut TestingT) {
    for disable_source_of_project_reference_redirect in [false, true] {
        let tsc_line = if disable_source_of_project_reference_redirect {
            "// @tsc: --build /src/tsconfig.json"
        } else {
            ""
        };
        let content = format!(
            r#"
// @stateBaseline: true
{tsc_line}
// @Filename: src/common/input/keyboard.ts
function bar() {{ return "just a random function so .d.ts location doesnt match"; }}
export function /*keyboard*/evaluateKeyboardEvent() {{ }}
// @Filename: src/common/input/keyboard.test.ts
import {{ evaluateKeyboardEvent }} from 'common/input/keyboard';
function testEvaluateKeyboardEvent() {{
	return evaluateKeyboardEvent();
}}
// @Filename: src/terminal.ts
/*terminal*/import {{ evaluateKeyboardEvent }} from 'common/input/keyboard';
function foo() {{
	return evaluateKeyboardEvent();
}}
// @Filename: /src/common/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true,
		"declarationMap": true,
		"outDir": "../../out",
		"disableSourceOfProjectReferenceRedirect": {disable_source_of_project_reference_redirect},
		"paths": {{
			"*": ["../*"],
		}},
	}},
	"include": ["./\**/*"]
}}
// @Filename: src/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true,
		"declarationMap": true,
		"outDir": "../out",
		"disableSourceOfProjectReferenceRedirect": {disable_source_of_project_reference_redirect},
		"paths": {{
			"common/*": ["./common/*"],
		}},
		"tsBuildInfoFile": "../out/src.tsconfig.tsbuildinfo"
	}},
	"include": ["./\**/*"],
	"references": [
		{{ "path": "./common" }},
	],
}}"#
        );
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
        f.go_to_marker(t, "keyboard");
        f.go_to_marker(t, "terminal");
        // Find all ref in default project
        f.verify_baseline_find_all_references(t, &["keyboard".to_string()]);
        done();
    }
}

pub fn test_find_all_refs_ancestor_sibling_projects_loading(t: &mut TestingT) {
    for disable_solution_searching in [false, true] {
        let content = format!(
            r#"
// @stateBaseline: true
// @Filename: solution/tsconfig.json
{{
	"files": [],
	"include": [],
	"references": [
		{{ "path": "./compiler" }},
		{{ "path": "./services" }},
	],
}}
// @Filename: solution/compiler/tsconfig.json
{{
	"compilerOptions": {{ 
		"composite": true,
		"disableSolutionSearching": {disable_solution_searching},
	}},
	"files": ["./types.ts", "./program.ts"]
}}
// @Filename: solution/compiler/types.ts
namespace ts {{
	export interface Program {{
		getSourceFiles(): string[];
	}}
}}
// @Filename: solution/compiler/program.ts
namespace ts {{
	export const program: Program = {{
		/*notLocal*/getSourceFiles: () => [/*local*/getSourceFile()]
	}};
	function getSourceFile() {{ return "something"; }}
}}
// @Filename: solution/services/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true
	}},
	"files": ["./services.ts"],
	"references": [
		{{ "path": "../compiler" }},
	],
}}
// @Filename: solution/services/services.ts
/// <reference path="../compiler/types.ts" />
/// <reference path="../compiler/program.ts" />
namespace ts {{
	const result = program.getSourceFiles();
}}"#
        );
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
        // Find all references for getSourceFile
        // Shouldnt load more projects
        f.verify_baseline_find_all_references(t, &["local".to_string()]);

        // Find all references for getSourceFiles
        // Should load more projects only if disableSolutionSearching is not set to true
        f.verify_baseline_find_all_references(t, &["notLocal".to_string()]);
        done();
    }
}

pub fn test_find_all_refs_overlapping_projects(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true 
// @Filename: solution/tsconfig.json
{
	"files": [],
	"include": [],
	"references": [
		{ "path": "./a" },
		{ "path": "./b" },
		{ "path": "./c" },
		{ "path": "./d" },
	],
}
// @Filename: solution/a/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"files": ["./index.ts"]
}
// @Filename: solution/a/index.ts
export interface I {
	M(): void;
}
// @Filename: solution/b/tsconfig.json
{
	"compilerOptions": {
		"composite": true
	},
	"files": ["./index.ts"],
	"references": [
		{ "path": "../a" },
	],
}
// @Filename: solution/b/index.ts
import { I } from "../a";
export class B implements /**/I {
	M() {}
}
// @Filename: solution/c/tsconfig.json
{
	"compilerOptions": {
		"composite": true
	},
	"files": ["./index.ts"],
	"references": [
		{ "path": "../b" },
	],
}
// @Filename: solution/c/index.ts
import { I } from "../a";
import { B } from "../b";
export const C: I = new B();
// @Filename: solution/d/tsconfig.json
{
	"compilerOptions": {
		"composite": true
	},
	"files": ["./index.ts"],
	"references": [
		{ "path": "../c" },
	],
}
// @Filename: solution/d/index.ts
import { I } from "../a";
import { C } from "../c";
export const D: I = C;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // The first search will trigger project loads
    f.verify_baseline_find_all_references(t, &["".to_string()]);

    // The second search starts with the projects already loaded
    // Formerly, this would search some projects multiple times
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}

pub fn test_find_all_refs_two_projects_open_and_one_project_references(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true
// @Filename: /myproject/main/src/file1.ts
/*main*/export const mainConst = 10;
// @Filename: /myproject/main/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"references": [
		{ "path": "../core" },
		{ "path": "../indirect" },
		{ "path": "../noCoreRef1" },
		{ "path": "../indirectDisabledChildLoad1" },
		{ "path": "../indirectDisabledChildLoad2" },
		{ "path": "../refToCoreRef3" },
		{ "path": "../indirectNoCoreRef" }
	]
}
// @Filename: /myproject/core/src/file1.ts
export const /*find*/coreConst = 10;
// @Filename: /myproject/core/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
}
// @Filename: /myproject/noCoreRef1/src/file1.ts
export const noCoreRef1Const = 10;
// @Filename: /myproject/noCoreRef1/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
}
// @Filename: /myproject/indirect/src/file1.ts
export const indirectConst = 10;
// @Filename: /myproject/indirect/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"references": [
		{ "path": "../coreRef1" },
	]
}
// @Filename: /myproject/coreRef1/src/file1.ts
export const coreRef1Const = 10;
// @Filename: /myproject/coreRef1/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"references": [
		{ "path": "../core" },
	]
}
// @Filename: /myproject/indirectDisabledChildLoad1/src/file1.ts
export const indirectDisabledChildLoad1Const = 10;
// @Filename: /myproject/indirectDisabledChildLoad1/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
		"disableReferencedProjectLoad": true,
	},
	"references": [
		{ "path": "../coreRef2" },
	]
}
// @Filename: /myproject/coreRef2/src/file1.ts
export const coreRef2Const = 10;
// @Filename: /myproject/coreRef2/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"references": [
		{ "path": "../core" },
	]
}
// @Filename: /myproject/indirectDisabledChildLoad2/src/file1.ts
export const indirectDisabledChildLoad2Const = 10;
// @Filename: /myproject/indirectDisabledChildLoad2/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
		"disableReferencedProjectLoad": true,
	},
	"references": [
		{ "path": "../coreRef3" },
	]
}
// @Filename: /myproject/coreRef3/src/file1.ts
export const coreRef3Const = 10;
// @Filename: /myproject/coreRef3/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"references": [
		{ "path": "../core" },
	]
}
// @Filename: /myproject/refToCoreRef3/src/file1.ts
export const refToCoreRef3Const = 10;
// @Filename: /myproject/refToCoreRef3/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"references": [
		{ "path": "../coreRef3" },
	]
}
// @Filename: /myproject/indirectNoCoreRef/src/file1.ts
export const indirectNoCoreRefConst = 10;
// @Filename: /myproject/indirectNoCoreRef/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
	"references": [
		{ "path": "../noCoreRef2" },
	]
}
// @Filename: /myproject/noCoreRef2/src/file1.ts
export const noCoreRef2Const = 10;
// @Filename: /myproject/noCoreRef2/tsconfig.json
{
	"compilerOptions": {
		"composite": true,
	},
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "main");
    f.verify_baseline_find_all_references(t, &["find".to_string()]);
    done();
}

pub fn test_find_all_refs_does_not_try_to_search_project_after_its_update_does_not_include_the_file(
    t: &mut TestingT,
) {
    let content = r#"
// @stateBaseline: true 
// @Filename: /packages/babel-loader/tsconfig.json
{
	"compilerOptions": {
		"target": "ES2018",
		"module": "commonjs",
		"strict": true,
		"esModuleInterop": true,
		"composite": true,
		"rootDir": "src",
		"outDir": "dist"
	},
	"include": ["src"],
	"references": [{"path": "../core"}]
}
// @Filename: /packages/babel-loader/src/index.ts
/*change*/import type { Foo } from "../../core/src/index.js";
// @Filename: /packages/core/tsconfig.json
{
	"compilerOptions": {
		"target": "ES2018",
		"module": "commonjs",
		"strict": true,
		"esModuleInterop": true,
		"composite": true,
		"rootDir": "./src",
		"outDir": "./dist",
	},
	"include": ["./src"]
}
// @Filename: /packages/core/src/index.ts
import { Bar } from "./loading-indicator.js";
export type Foo = {};
const bar: Bar = {
	/*prop*/prop: 0
}
// @Filename: /packages/core/src/loading-indicator.ts
export interface Bar {
	prop: number;
}
const bar: Bar = {
	prop: 1
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "change");
    f.go_to_marker(t, "prop");

    // Now change `babel-loader` project to no longer import `core` project
    f.go_to_marker(t, "change");
    f.insert(t, "// comment");

    // At this point, we haven't updated `babel-loader` project yet,
    // so `babel-loader` is still a containing project of `loading-indicator` file.
    // When calling find all references,
    // we shouldn't crash due to using outdated information on a file's containing projects.
    f.verify_baseline_find_all_references(t, &["prop".to_string()]);
    done();
}

pub fn test_find_all_refs_open_file_in_configured_project_that_will_be_removed(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true
// @Filename: /myproject/playground/tsconfig.json
{}
// @Filename: /myproject/playground/tests.ts
/*tests*/export function foo() {}
// @Filename: /myproject/playground/tsconfig-json/tsconfig.json
{
	"include": ["./src"]
}
// @Filename: /myproject/playground/tsconfig-json/src/src.ts
export function foobar() {}
// @Filename: /myproject/playground/tsconfig-json/tests/spec.ts
export function /*find*/bar() { }
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "tests");
    f.close_file_of_marker(t, "tests");
    f.verify_baseline_find_all_references(t, &["find".to_string()]);
    done();
}

pub fn test_find_all_refs_special_handling_of_localness(t: &mut TestingT) {
    struct TestCase {
        name: &'static str,
        definition: &'static str,
        usage: &'static str,
        reference_term: &'static str,
    }

    let tests = [
        TestCase {
            name: "ArrowFunctionAssignment",
            definition: r#"export const dog = () => { };"#,
            usage: r#"shared.dog();"#,
            reference_term: "dog",
        },
        TestCase {
            name: "ArrowFunctionAsObjectLiteralPropertyTypes",
            definition: r#"export const foo = { bar: () => { } };"#,
            usage: r#"shared.foo.bar();"#,
            reference_term: "bar",
        },
        TestCase {
            name: "ObjectLiteralProperty",
            definition: r#"export const foo = {  baz: "BAZ" };"#,
            usage: r#"shared.foo.baz;"#,
            reference_term: "baz",
        },
        TestCase {
            name: "MethodOfClassExpression",
            definition: r#"export const foo = class { fly() {} };"#,
            usage: r#"const instance = new shared.foo();
instance.fly();"#,
            reference_term: "fly",
        },
        TestCase {
            // when using arrow function as object literal property is loaded through indirect assignment with original declaration local to project is treated as local
            name: "ArrowFunctionAsObjectLiteralProperty",
            definition: r#"const local = { bar: () => { } };
export const foo = local;"#,
            usage: r#"shared.foo.bar();"#,
            reference_term: "bar",
        },
    ];
    for tc in tests {
        let reference_index = tc.usage.find(tc.reference_term).unwrap_or(0);
        let usage_with_marker = format!(
            "{}/*ref*/{}",
            &tc.usage[..reference_index],
            &tc.usage[reference_index..]
        );
        let content = format!(
            r#"
// @stateBaseline: true
// @Filename: /solution/tsconfig.json
{{
	"files": [],
	"references": [
		{{ "path": "./api" }},
		{{ "path": "./app" }},
	],
}}
// @Filename: /solution/api/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true,
		"outDir": "dist",
		"rootDir": "src"
	}},
	"include": ["src"],
	"references": [{{ "path": "../shared" }}],
}}
// @Filename: /solution/api/src/server.ts
import * as shared from "../../shared/dist"
{usage_with_marker}
// @Filename: /solution/app/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true,
		"outDir": "dist",
		"rootDir": "src"
	}},
	"include": ["src"],
	"references": [{{ "path": "../shared" }}],
}}
// @Filename: /solution/app/src/app.ts
import * as shared from "../../shared/dist"
{}
// @Filename: /solution/app/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true,
		"outDir": "dist",
		"rootDir": "src"
	}},
	"include": ["src"],
	"references": [{{ "path": "../shared" }}],
}}
// @Filename: /solution/shared/tsconfig.json
{{
    "compilerOptions": {{
        "composite": true,
        "outDir": "dist",
        "rootDir": "src"
    }},
    "include": ["src"],
}}
// @Filename: /solution/shared/src/index.ts
{}"#,
            tc.usage, tc.definition
        );
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
        f.verify_baseline_find_all_references(t, &["ref".to_string()]);
        done();
        let _ = tc.name;
    }
}

pub fn test_find_all_refs_re_export_in_multi_project_solution(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true
// @Filename: /tsconfig.base.json
{
	"compilerOptions": {
		"rootDir": ".",
		"outDir": "target",
		"module": "ESNext",
		"moduleResolution": "bundler",
		"composite": true,
		"declaration": true,
		"strict": true
	},
	"include": []
}
// @Filename: /tsconfig.json
{
	"extends": "./tsconfig.base.json",
	"references": [
		{ "path": "project-a" },
		{ "path": "project-b" },
		{ "path": "project-c" },
	]
}
// @Filename: /project-a/tsconfig.json
{
	"extends": "../tsconfig.base.json",
	"include": ["*"]
}
// @Filename: /project-a/private.ts
export const /*symbolA*/symbolA = 'some-symbol';
console.log(symbolA);
// @Filename: /project-a/public.ts
export { symbolA } from './private';
// @Filename: /project-b/tsconfig.json
{
	"extends": "../tsconfig.base.json",
	"include": ["*"]
}
// @Filename: /project-b/public.ts
export const /*symbolB*/symbolB = 'symbol-b';
// @Filename: /project-c/tsconfig.json
{
	"extends": "../tsconfig.base.json",
	"include": ["*"],
	"references": [
		{ "path": "../project-a" },
		{ "path": "../project-b" },
	]
}
// @Filename: /project-c/index.ts
import { symbolB } from '../project-b/public';
import { /*symbolAUsage*/symbolA } from '../project-a/public';
console.log(symbolB);
console.log(symbolA);
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // Find all refs for symbolA - should find definition in private.ts, re-export in public.ts, and usage in project-c/index.ts
    f.verify_baseline_find_all_references(t, &["symbolA".to_string()]);

    // Find all refs for symbolB - should find definition and usage (no re-export involved)
    f.verify_baseline_find_all_references(t, &["symbolB".to_string()]);

    // Find all refs from the usage site - should also work
    f.verify_baseline_find_all_references(t, &["symbolAUsage".to_string()]);
    done();
}

pub fn test_find_all_refs_declaration_in_other_project(t: &mut TestingT) {
    struct TestCase {
        project_already_loaded: bool,
        disable_referenced_project_load: bool,
        disable_source_of_project_reference_redirect: bool,
        dts_map_present: bool,
    }
    // Pre-loaded = A file from project B is already open when FindAllRefs is invoked
    // dRPL = Project A has disableReferencedProjectLoad
    // dSOPRR = Project A has disableSourceOfProjectReferenceRedirect
    // Map = The declaration map file b/lib/index.d.ts.map exists
    // B refs = files under directory b in which references are found (all scenarios find all references in a/index.ts)
    //Pre-loaded |dRPL|dSOPRR|Map|     B state      | Notes        | B refs              | Notes
    //-----------+----+------+- -+------------------+--------------+---------------------+---------------------------------------------------
    let tests = [
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: false,
        },
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: false,
        },
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: false,
        },
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: true,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: false,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: false,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: true,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: false,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: true,
            dts_map_present: false,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: true,
        },
        TestCase {
            project_already_loaded: false,
            disable_referenced_project_load: false,
            disable_source_of_project_reference_redirect: false,
            dts_map_present: false,
        },
    ];
    for tc in tests {
        let mut content = format!(
            r#"
// @stateBaseline: true
// @Filename: /myproject/a/tsconfig.json
{{
	"disableReferencedProjectLoad": {},
	"disableSourceOfProjectReferenceRedirect": {},
	"composite": true
}}
// @Filename: /myproject/a/index.ts
import {{ B }} from "../b/lib";
const b: /*ref*/B = new B();
// @Filename: /myproject/b/tsconfig.json
{{
	"declarationMap": true,
	"outDir": "lib",
	"composite": true,
}}
// @Filename: /myproject/b/index.ts
export class B {{
	M() {{}}
}}
// @Filename: /myproject/b/helper.ts
/*bHelper*/import {{ B }} from ".";
const b: B = new B();
// @Filename: /myproject/b/lib/index.d.ts
export declare class B {{
	M(): void;
}}
//# sourceMappingURL=index.d.ts.map"#,
            tc.disable_referenced_project_load, tc.disable_source_of_project_reference_redirect
        );
        if tc.dts_map_present {
            content.push_str(
                r#"
// @Filename: /myproject/b/lib/index.d.ts.map
{
	"version": 3,
	"file": "index.d.ts",
	"sourceRoot": "",
	"sources": ["../index.ts"],
	"names": [],
	"mappings": "AAAA,qBAAa,CAAC;IACV,CAAC;CACJ"
}"#,
            );
        }
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
        if tc.project_already_loaded {
            f.go_to_marker(t, "ref");
            f.go_to_marker(t, "bHelper");
        }
        f.verify_baseline_find_all_references(t, &["ref".to_string()]);
        done();
    }
}

fn run_default_project_solution_scenario(
    t: &mut TestingT,
    indirect: bool,
    disable_referenced_project_load_on_root: bool,
    disable_referenced_project_load_on_indirect1: bool,
    disable_referenced_project_load_on_indirect2: bool,
    own_files: bool,
) {
    let content = default_project_solution_content(
        indirect,
        disable_referenced_project_load_on_root,
        disable_referenced_project_load_on_indirect1,
        disable_referenced_project_load_on_indirect2,
        own_files,
    );
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
    // Ensure configured project is found for open file
    f.go_to_marker(t, "mainFoo");
    // !!! TODO Verify errors
    f.go_to_marker(t, "dummy");

    // Projects lifetime
    f.close_file_of_marker(t, "dummy");
    f.close_file_of_marker(t, "mainFoo");
    f.go_to_marker(t, "dummy");

    f.close_file_of_marker(t, "dummy");

    // Find all refs in default project
    f.verify_baseline_find_all_references(t, &["mainFoo".to_string()]);

    f.close_file_of_marker(t, "mainFoo");

    // Find all ref in non default project
    f.verify_baseline_find_all_references(t, &["fooIndirect3Import".to_string()]);
    done();
}

fn default_project_solution_content(
    indirect: bool,
    disable_referenced_project_load_on_root: bool,
    disable_referenced_project_load_on_indirect1: bool,
    disable_referenced_project_load_on_indirect2: bool,
    own_files: bool,
) -> String {
    let root_compiler_options = if disable_referenced_project_load_on_root {
        r#"
	"compilerOptions": {
		"disableReferencedProjectLoad": true
	},"#
    } else {
        ""
    };
    let references = if indirect {
        r#""references":  [
		{ "path": "./tsconfig-indirect1.json" },
		{ "path": "./tsconfig-indirect2.json" },
	]"#
    } else {
        r#""references": [{ "path": "./tsconfig-src.json" }]"#
    };
    let root_files = if own_files {
        r#""files": ["./own/main.ts"],"#
    } else {
        r#""files": [],"#
    };
    let own_project = if own_files {
        r#"
// @Filename: myproject/own/main.ts
import { foo } from '../target/src/main';
foo();
export function bar() {}"#
    } else {
        ""
    };
    let indirect_projects = if indirect {
        format!(
            r#"
// @FileName: myproject/indirect1/main.ts
export const indirect = 1;
// @Filename: myproject/tsconfig-indirect1.json
{{
	"compilerOptions": {{
		"composite": true,
		"outDir": "./target/",
		{}
	}},
	"files": [
		"./indirect1/main.ts"
	],
	"references": [
		{{
			"path": "./tsconfig-src.json"
		}}
	]
}}
// @FileName: myproject/indirect2/main.ts
export const indirect = 1;
// @Filename: myproject/tsconfig-indirect2.json
{{
	"compilerOptions": {{
		"composite": true,
		"outDir": "./target/",
		{}
	}},
	"files": [
		"./indirect2/main.ts"
	],
	"references": [
		{{
			"path": "./tsconfig-src.json"
		}}
	]
}}"#,
            if disable_referenced_project_load_on_indirect1 {
                r#""disableReferencedProjectLoad": true,"#
            } else {
                ""
            },
            if disable_referenced_project_load_on_indirect2 {
                r#""disableReferencedProjectLoad": true,"#
            } else {
                ""
            }
        )
    } else {
        String::new()
    };
    format!(
        r#"
// @stateBaseline: true 
// @tsc: --build /myproject/tsconfig.json
// @Filename: dummy/dummy.ts
/*dummy*/const x = 1;
// @Filename: dummy/tsconfig.json
{{ }}
// @Filename: myproject/tsconfig.json
{{
	{root_compiler_options}
	{root_files}
	{references}
}}
{own_project}
// @Filename: myproject/tsconfig-src.json
{{
	"compilerOptions": {{
		"composite": true,
		"outDir": "./target",
		"declarationMap": true,
	}},
	"include": ["./src/\**/*"]
}}
// @Filename: myproject/src/main.ts
import {{ foo }} from './helpers/functions';
export {{ /*mainFoo*/foo }};
// @Filename: myproject/src/helpers/functions.ts
export function foo() {{ return 1; }}
// @Filename: myproject/indirect3/tsconfig.json
{{ }}
// @Filename: myproject/indirect3/main.ts
import {{ /*fooIndirect3Import*/foo }} from '../target/src/main';
foo()
export function bar() {{}}
{indirect_projects}"#
    )
}

