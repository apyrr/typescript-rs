#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_nonrelative_import_typings3() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_nonrelative_import_typings3(&mut t);
}

fn run_test_completion_for_string_literal_nonrelative_import_typings3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: subdirectory/test0.ts
/// <reference types="m/*types_ref0*/" />
import * as foo1 from "m/*import_as0*/
import foo2 = require("m/*import_equals0*/
var foo3 = require("m/*require0*/
// @Filename: subdirectory/node_modules/@types/module-x/index.d.ts
export var x = 9;
// @Filename: subdirectory/package.json
{ "dependencies": { "@types/module-x": "latest" } }
// @Filename: node_modules/@types/module-y/index.d.ts
export var y = 9;
// @Filename: package.json
{ "dependencies": { "@types/module-y": "latest" } }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "types_ref0".to_string(),
            "import_as0".to_string(),
            "import_equals0".to_string(),
            "require0".to_string(),
        ]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("module-x".to_string()),
                    CompletionsExpectedItem::Label("module-y".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
