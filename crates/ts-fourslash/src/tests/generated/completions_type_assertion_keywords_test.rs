#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_type_assertion_keywords() {
    let mut t = TestingT;
    run_test_completions_type_assertion_keywords(&mut t);
}

fn run_test_completions_type_assertion_keywords(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsTypeAssertionKeywords") {
        return;
    }
    let content = r"// @lib: es5
const a = {
  b: 42 as /*0*/
};

1 as /*1*/

const b = 42 as /*2*/

var c = </*3*/>42";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Markers(f.markers()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_type_assertion_keywords(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
