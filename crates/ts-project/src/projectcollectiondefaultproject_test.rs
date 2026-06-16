use std::collections::HashMap;

use crate::projecttestutil;
use ts_core as core;
use ts_lsproto as lsproto;

const LANGUAGE_KIND_TYPESCRIPT: &str = "typescript";

#[test]
fn test_project_collection_default_project() {
    if !ts_bundled::EMBEDDED {
        return;
    }

    // Project 1 references project 2, which does not have open files.
    // File project1/dist/index.d.ts does not belong to any tsconfig.json, but is included in programs for
    // projects 3 and 4 via project 1's output.
    // When looking for a default project for project1/dist/index.d.ts,
    // we should not try to unconditionally access project 2,
    // which isn't loaded because of `disableReferencedProjectLoad`.
    let files = HashMap::from([
        (
            "/project1/tsconfig.json".to_string(),
            r#"{
			"extends": "../tsconfig.json",
			"files": [],
			"include": ["src/**/*"],
			"references": [
				{
					"path": "../project2"
				}
			],
			"compilerOptions": {
				"composite": true,
				"outDir": "./dist",
				"rootDir": "./src",
			}
		}"#
            .to_string(),
        ),
        (
            "/project1/src/index.ts".to_string(),
            r#"export const foo = 42;
		export type Bar = { a: string };"#
                .to_string(),
        ),
        (
            "/project1/dist/index.d.ts".to_string(),
            r#"export declare const foo = 42;
			export type Bar = {
				a: string;
			};"#
            .to_string(),
        ),
        (
            "/project2/tsconfig.json".to_string(),
            r#"{
			"extends": "../tsconfig.json",
			"files": [],
			"include": ["src/**/*"],
			"compilerOptions": {
				"composite": true,
				"outDir": "./dist",
				"rootDir": "./src"
			}
		}"#
            .to_string(),
        ),
        (
            "/project3/tsconfig.json".to_string(),
            r#"{
			"extends": "../tsconfig.json",
			"files": [],
			"include": ["src/**/*"],
			"references": [
				{
					"path": "../project1"
				}
			],
			"compilerOptions": {
				"composite": true,
				"outDir": "./dist",
				"rootDir": "./src",
			}
		}"#
            .to_string(),
        ),
        (
            "/project3/src/index.ts".to_string(),
            r#"import { Bar } from "../../project1/dist/index.js";
			declare const b: Bar;
			const x: string = b.a;"#
                .to_string(),
        ),
        (
            "/project4/tsconfig.json".to_string(),
            r#"{
			"extends": "../tsconfig.json",
			"files": [],
			"include": ["src/**/*"],
			"references": [
				{
					"path": "../project1"
				}
			],
			"compilerOptions": {
				"composite": true,
				"outDir": "./dist",
				"rootDir": "./src",
			}
		}"#
            .to_string(),
        ),
        (
            "/project4/src/index.ts".to_string(),
            r#"import { Bar } from "../../project1/dist/index.js";
declare const b: Bar;
const x: string = b.a;"#
                .to_string(),
        ),
        (
            "/tsconfig.json".to_string(),
            r#"{
			"compilerOptions": {
				"disableReferencedProjectLoad": true,
				"disableSolutionSearching": true,
				"disableSourceOfProjectReferenceRedirect": true
			},
			"files": [],
			"references": [
				{
					"path": "./project1"
				},
				{
					"path": "./project2"
				},
				{
					"path": "./project3"
				},
				{
					"path": "./project4"
				}
			]
		}"#
            .to_string(),
        ),
    ]);
    let uris = [
        lsproto::DocumentUri::from("file:///project1/dist/index.d.ts"),
        lsproto::DocumentUri::from("file:///project1/src/index.ts"),
        lsproto::DocumentUri::from("file:///project3/src/index.ts"),
        lsproto::DocumentUri::from("file:///project4/src/index.ts"),
    ];
    let (mut session, _) = projecttestutil::setup(files.clone());
    // Should not crash.
    for uri in uris {
        let content = files[&uri["file://".len()..]].clone();
        session.did_open_file(
            core::Context::default(),
            uri,
            1,
            content,
            LANGUAGE_KIND_TYPESCRIPT.to_string(),
        );
    }
}
