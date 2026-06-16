#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_paths_json_module_with_amd() {
    let mut t = TestingT;
    run_test_completions_paths_json_module_with_amd(&mut t);
}

fn run_test_completions_paths_json_module_with_amd(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsPathsJsonModuleWithAmd") {
        return;
    }
    let content = r#"// @module: amd
// @resolveJsonModule: true
// @Filename: /project/test.json
not read
// @Filename: /project/index.ts
import { } from ".//**/";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
