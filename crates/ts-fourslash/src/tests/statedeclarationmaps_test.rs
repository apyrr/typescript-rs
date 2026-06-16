use crate::{new_fourslash, TestingT};

pub fn test_declaration_maps_opening_original_location_project(t: &mut TestingT) {
    for disable_source_of_project_reference_redirect in [false, true] {
        let content = format!(
            r#"
// @stateBaseline: true
// @Filename: a/a.ts
export class A {{ }}
// @Filename: a/tsconfig.json
{{}}
// @Filename: a/a.d.ts
export declare class A {{
}}
//# sourceMappingURL=a.d.ts.map
// @Filename: a/a.d.ts.map
{{
	"version": 3,
	"file": "a.d.ts",
	"sourceRoot": "",
	"sources": ["./a.ts"],
	"names": [],
	"mappings": "AAAA,qBAAa,CAAC;CAAI"
}}
// @Filename: b/b.ts
import {{A}} from "../a/a";
new /*1*/A();
// @Filename: b/tsconfig.json
{{
	"compilerOptions": {{
		"disableSourceOfProjectReferenceRedirect": {disable_source_of_project_reference_redirect}
	}},
	"references": [
		{{ "path": "../a" }}
	]
}}"#
        );
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
        f.verify_baseline_find_all_references(t, &["1".to_string()]);
        done();
    }
}

pub fn test_declaration_map_test_cases_for_maps(t: &mut TestingT) {
    struct TestCase {
        name: &'static str,
        go_to_marker: &'static str,
        op_marker: &'static str,
    }
    let tests = [
        TestCase {
            name: "FindAllRefs",
            go_to_marker: "userFnA",
            op_marker: "userFnA",
        },
        TestCase {
            name: "FindAllRefsStartingAtDefinition",
            go_to_marker: "userFnA",
            op_marker: "fnADef",
        },
        TestCase {
            name: "FindAllRefsTargetDoesNotExist",
            go_to_marker: "userFnB",
            op_marker: "userFnB",
        },
        TestCase {
            name: "Rename",
            go_to_marker: "userFnA",
            op_marker: "userFnA",
        },
        TestCase {
            name: "RenameStartingAtDefinition",
            go_to_marker: "userFnA",
            op_marker: "fnADef",
        },
        TestCase {
            name: "RenameTargetDoesNotExist",
            go_to_marker: "userFnB",
            op_marker: "userFnB",
        },
    ];
    for tc in tests {
        let content = declaration_map_test_cases_content();
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
        f.go_to_marker(t, tc.go_to_marker);
        // Ref projects are loaded after as part of this command
        if tc.name.starts_with("Rename") {
            f.verify_baseline_rename(t, &[tc.op_marker.to_string()]);
        } else {
            f.verify_baseline_find_all_references(t, &[tc.op_marker.to_string()]);
        }
        // Open temp file and verify all projects alive
        f.close_file_of_marker(t, tc.go_to_marker);
        f.go_to_marker(t, "dummy");
        done();
    }
}

pub fn test_declaration_maps_workspace_symbols(t: &mut TestingT) {
    let content = r#"// @stateBaseline: true
// @Filename: a/a.ts
export function fnA() {}
export interface IfaceA {}
export const instanceA: IfaceA = {};
// @Filename: a/tsconfig.json
{
	"compilerOptions": {
		"outDir": "bin",
		"declarationMap": true,
		"composite": true
	}
}
// @Filename: a/bin/a.d.ts.map
{
	"version": 3,
	"file": "a.d.ts",
	"sourceRoot": "",
	"sources": ["../a.ts"],
	"names": [],
	"mappings": "AAAA,wBAAgB,GAAG,SAAK;AACxB,MAAM,WAAW,MAAM;CAAG;AAC1B,eAAO,MAAM,SAAS,EAAE,MAAW,CAAC"
}
// @Filename: a/bin/a.d.ts
export declare function fnA(): void;
export interface IfaceA {
}
export declare const instanceA: IfaceA;
//# sourceMappingURL=a.d.ts.map
// @Filename: b/b.ts
export function fnB() {}
// @Filename: b/c.ts
export function fnC() {}
// @Filename: b/tsconfig.json
{
	"compilerOptions": {
		"outDir": "bin",
		"declarationMap": true,
		"composite": true
	}
}
// @Filename: b/bin/b.d.ts.map
{
	"version": 3,
	"file": "b.d.ts",
	"sourceRoot": "",
	"sources": ["../b.ts"],
	"names": [],
	"mappings": "AAAA,wBAAgB,GAAG,SAAK"
}
// @Filename: b/bin/b.d.ts
export declare function fnB(): void;
//# sourceMappingURL=b.d.ts.map
// @Filename: user/user.ts
/*user*/import * as a from "../a/a";
import * as b from "../b/b";
export function fnUser() {
	a.fnA();
	b.fnB();
	a.instanceA;
}
// @Filename: user/tsconfig.json
{
	"references": [
		{ "path": "../a" },
		{ "path": "../b" }
	]
}
// @Filename: dummy/dummy.ts
/*dummy*/export const a = 10;
// @Filename: dummy/tsconfig.json
{}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "user");
    // Ref projects are loaded after as part of this command
    f.verify_baseline_workspace_symbol(t, "fn");
    // Open temp file and verify all projects alive
    f.close_file_of_marker(t, "user");
    f.go_to_marker(t, "dummy");
    done();
}

pub fn test_declaration_maps_find_all_refs_definition_in_mapped_file(t: &mut TestingT) {
    let content = r#"
// @stateBaseline: true 
//@Filename: a/a.ts
export function f() {}
// @Filename: a/tsconfig.json
{
	"compilerOptions": {
		"outDir": "../bin",
		"declarationMap": true,
		"composite": true
	}
}
//@Filename: b/b.ts
import { f } from "../bin/a";
/*1*/f();
// @Filename: b/tsconfig.json
{
	"references": [
		{ "path": "../a" }
	]
}
// @Filename: bin/a.d.ts
export declare function f(): void;
//# sourceMappingURL=a.d.ts.map
// @Filename: bin/a.d.ts.map
{
	"version":3,
	"file":"a.d.ts",
	"sourceRoot":"",
	"sources":["a.ts"],
	"names":[],
	"mappings":"AAAA,wBAAgB,CAAC,SAAK"
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}

pub fn test_declaration_maps_rename(t: &mut TestingT) {
    let tests = [
        TestCase {
            name: "ProjectReferences",
            dont_build: true,
            main_with_no_ref: false,
            disable_source_of_project_reference_redirect: false,
            tsconfig_not_solution: false,
        },
        TestCase {
            name: "DisableSourceOfProjectReferenceRedirect",
            dont_build: false,
            main_with_no_ref: false,
            disable_source_of_project_reference_redirect: true,
            tsconfig_not_solution: false,
        },
        TestCase {
            name: "SourceMaps",
            dont_build: false,
            main_with_no_ref: true,
            disable_source_of_project_reference_redirect: false,
            tsconfig_not_solution: false,
        },
        TestCase {
            name: "SourceMapsNotSolution",
            dont_build: false,
            main_with_no_ref: true,
            disable_source_of_project_reference_redirect: false,
            tsconfig_not_solution: true,
        },
    ];
    for tc in tests {
        let content = declaration_maps_rename_content(&tc);
        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.clone());
        f.go_to_marker(t, "dummy");
        // Ref projects are loaded after as part of this command
        f.verify_baseline_rename(t, &["rename".to_string()]);
        // Collecting at this point retains dependency.d.ts and map
        f.close_file_of_marker(t, "dummy");
        f.go_to_marker(t, "dummy");
        // Closing open file, removes dependencies too
        f.close_file_of_marker(t, "rename");
        f.close_file_of_marker(t, "dummy");
        f.go_to_marker(t, "dummy");
        done();

        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.clone());
        // Ref projects are loaded after as part of this command
        f.verify_baseline_rename(t, &["rename".to_string()]);
        f.go_to_marker(t, "firstLine");
        f.insert(t, "function fooBar() { }\n");
        f.verify_baseline_rename(t, &["rename".to_string()]);
        done();

        let (mut f, done) = new_fourslash(t, None /*capabilities*/, content);
        // Ref projects are loaded after as part of this command
        f.verify_baseline_rename(t, &["rename".to_string()]);
        f.go_to_marker(t, "lastLine");
        f.insert(t, "const x = 10;");
        f.verify_baseline_rename(t, &["rename".to_string()]);
        done();
        let _ = tc.name;
    }
}

// TestDeclarationMapsNonMonotonicMappings verifies that getMappedLocation clamps
// inverted ranges caused by non-monotonic source map mappings.
//
// The baseline comparison catches regressions: without the fix, the output shows
// inverted markers (e.g., "|]|>" appearing before "<|"), while with the fix,
// the ranges are clamped to valid zero-length ranges (e.g., "<||>").
pub fn test_declaration_maps_non_monotonic_mappings(t: &mut TestingT) {
    // The source map creates a non-monotonic mapping:
    // - .d.ts line 0 col 24 ('b' identifier) -> source line 1, col 16
    // - .d.ts line 0 col 25 (right after 'b') -> source line 0, col 0 (EARLIER!)
    //
    // When looking up 'b' identifier [24, 25), start maps to ~byte 39,
    // but end maps to byte 0, creating an inverted range.
    // The fix in getMappedLocation clamps this to prevent negative ranges.
    let content = r#"
// @Filename: /src/index.ts
export function a() {}
export function b() {}
// @Filename: /src/indexdef.d.ts.map
{
	"version": 3,
	"file": "indexdef.d.ts",
	"sourceRoot": "",
	"sources": ["index.ts"],
	"names": [],
	"mappings": "AACA,wBAAgB,CADhB;AAAA,wBAAgB"
}
// @Filename: /src/indexdef.d.ts
export declare function b(): void;
export declare function a(): void;
//# sourceMappingURL=indexdef.d.ts.map
// @Filename: /src/user.ts
import { a, b } from "./indexdef";
/*1*/a();
/*2*/b();
// @Filename: /src/tsconfig.json
{}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string(), "2".to_string()]);
    done();
}

fn declaration_map_test_cases_content() -> String {
    r#"
// @stateBaseline: true
// @Filename: a/a.ts
export function /*fnADef*/fnA() {}
export interface IfaceA {}
export const instanceA: IfaceA = {};
// @Filename: a/tsconfig.json
{
	"compilerOptions": {
		"outDir": "bin",
		"declarationMap": true,
		"composite": true
	}
}
// @Filename: a/bin/a.d.ts.map
{
	"version": 3,
	"file": "a.d.ts",
	"sourceRoot": "",
	"sources": ["../a.ts"],
	"names": [],
	"mappings": "AAAA,wBAAgB,GAAG,SAAK;AACxB,MAAM,WAAW,MAAM;CAAG;AAC1B,eAAO,MAAM,SAAS,EAAE,MAAW,CAAC"
}
// @Filename: a/bin/a.d.ts
export declare function fnA(): void;
export interface IfaceA {
}
export declare const instanceA: IfaceA;
//# sourceMappingURL=a.d.ts.map
// @Filename: b/tsconfig.json
{
	"compilerOptions": {
		"outDir": "bin",
		"declarationMap": true,
		"composite": true
	}
}
// @Filename: b/bin/b.d.ts.map
{
	"version": 3,
	"file": "b.d.ts",
	"sourceRoot": "",
	"sources": ["../b.ts"],
	"names": [],
	"mappings": "AAAA,wBAAgB,GAAG,SAAK"
}
// @Filename: b/bin/b.d.ts
export declare function fnB(): void;
//# sourceMappingURL=b.d.ts.map
// @Filename: user/user.ts
import * as a from "../a/bin/a";
import * as b from "../b/bin/b";
export function fnUser() { a./*userFnA*/fnA(); b./*userFnB*/fnB(); a.instanceA; }
// @Filename: dummy/dummy.ts
/*dummy*/export const a = 10;
// @Filename: dummy/tsconfig.json
{}"#
    .to_string()
}

fn declaration_maps_rename_content(tc: &TestCase) -> String {
    let build_str = if !tc.dont_build {
        "// @tsc: --build /myproject/dependency,--build /myproject/main"
    } else {
        ""
    };
    let main_refs_str = if !tc.main_with_no_ref {
        r#""references": [{ "path": "../dependency" }]"#
    } else {
        ""
    };
    let files_str = if !tc.tsconfig_not_solution {
        r#""files": [],"#
    } else {
        ""
    };
    format!(
        r#"
// @stateBaseline: true 
{build_str}
//@Filename: myproject/dependency/FnS.ts
/*firstLine*/export function fn1() {{ }}
export function fn2() {{ }}
export function /*rename*/fn3() {{ }}
export function fn4() {{ }}
export function fn5() {{ }}
/*lastLine*/
// @Filename: myproject/dependency/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true,
		"declarationMap": true,
		"declarationDir": "../decls"
	}}
}}
//@Filename: myproject/main/main.ts
import {{
	fn1,
	fn2,
	fn3,
	fn4,
	fn5
}} from "../decls/FnS";

fn1();
fn2();
fn3();
fn4();
fn5();
// @Filename: myproject/main/tsconfig.json
{{
	"compilerOptions": {{
		"composite": true,
		"declarationMap": true,
		"disableSourceOfProjectReferenceRedirect": {}
	}},
	{main_refs_str}
}}
// @Filename: myproject/tsconfig.json
{{
	"compilerOptions": {{
		"disableSourceOfProjectReferenceRedirect": {}
	}},
	{files_str}
	"references": [
		{{ "path": "dependency" }},
		{{ "path": "main" }}
	]
}}
// @Filename: random/random.ts
/*dummy*/export const a = 10;
// @Filename: random/tsconfig.json
{{}}"#,
        tc.disable_source_of_project_reference_redirect,
        tc.disable_source_of_project_reference_redirect
    )
}

struct TestCase {
    name: &'static str,
    dont_build: bool,
    main_with_no_ref: bool,
    disable_source_of_project_reference_redirect: bool,
    tsconfig_not_solution: bool,
}

