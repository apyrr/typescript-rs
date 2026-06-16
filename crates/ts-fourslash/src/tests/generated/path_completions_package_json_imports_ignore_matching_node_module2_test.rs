#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_path_completions_package_json_imports_ignore_matching_node_module2() {
    let mut t = TestingT;
    run_test_path_completions_package_json_imports_ignore_matching_node_module2(&mut t);
}

fn run_test_path_completions_package_json_imports_ignore_matching_node_module2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r##"// @module: node18
// @Filename: /package.json
{
  "imports": {
    "#internal/*": "./src/*.ts"
  }
}
// @Filename: /src/something.ts
export function something(name: string): any;
// @Filename: /src/node_modules/#internal/package.json
{}
// @Filename: /src/a.ts
import {} from "#internal//*1*/";"##;
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
                exact: vec![
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("something".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
