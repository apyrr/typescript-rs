#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_completions_package_json_exports_specifier_ends_in_ts() {
    let mut t = TestingT;
    run_test_import_completions_package_json_exports_specifier_ends_in_ts(&mut t);
}

fn run_test_import_completions_package_json_exports_specifier_ends_in_ts(t: &mut TestingT) {
    if should_skip_if_failing("TestImportCompletionsPackageJsonExportsSpecifierEndsInTs") {
        return;
    }
    let content = r#"// @module: node18
// @Filename: /node_modules/pkg/package.json
{
    "name": "pkg",
    "version": "1.0.0",
    "exports": {
      "./something.ts": "./a.js"
    }
 }
// @Filename: /node_modules/pkg/a.d.ts
export function foo(): void;
// @Filename: /package.json
{
    "dependencies": {
       "pkg": "*"
    }
 }
// @Filename: /index.ts
import {} from "pkg//*1*/";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("something.ts".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
