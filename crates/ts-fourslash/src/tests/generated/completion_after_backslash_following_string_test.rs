#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_after_backslash_following_string() {
    let mut t = TestingT;
    run_test_completion_after_backslash_following_string(&mut t);
}

fn run_test_completion_after_backslash_following_string(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionAfterBackslashFollowingString") {
        return;
    }
    let content = r#"// @lib: es5
Harness.newLine = ""\n/**/"#;
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
