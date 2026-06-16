#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_satisfies_tag_completion2() {
    let mut t = TestingT;
    run_test_jsdoc_satisfies_tag_completion2(&mut t);
}

fn run_test_jsdoc_satisfies_tag_completion2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocSatisfiesTagCompletion2") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJS: true
// @checkJs: true
// @filename: /a.js
/**
 * @/**/
 */
const t = { a: 1 };";
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
                includes: vec![CompletionsExpectedItem::Label("satisfies".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
