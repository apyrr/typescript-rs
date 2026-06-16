#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_completions_package_json_imports_conditions1() {
    let mut t = TestingT;
    run_test_import_completions_package_json_imports_conditions1(&mut t);
}

fn run_test_import_completions_package_json_imports_conditions1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r##"// @module: node18
// @Filename: /package.json
{
  "imports": {
    "#thing": {
        "types": { "import": "./types-esm/thing.d.mts", "require": "./types/thing.d.ts" },
        "default": { "import": "./esm/thing.mjs", "require": "./dist/thing.js" }
     }
  }
}
// @Filename: /types/thing.d.ts
export function something(name: string): any;
// @Filename: /src/foo.ts
import {} from "/*1*/";"##;
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
                exact: vec![CompletionsExpectedItem::Label("#thing".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
