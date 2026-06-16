#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_external_module_renamed_exports() {
    let mut t = TestingT;
    run_test_completions_external_module_renamed_exports(&mut t);
}

fn run_test_completions_external_module_renamed_exports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: other.ts
export {};
// @Filename: index.ts
const c = 0;
export { c as yeahThisIsTotallyInScopeHuh };
export * as alsoNotInScope from "./other";

/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("c".to_string())],
                excludes: vec![
                    "yeahThisIsTotallyInScopeHuh".to_string(),
                    "alsoNotInScope".to_string(),
                ],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
