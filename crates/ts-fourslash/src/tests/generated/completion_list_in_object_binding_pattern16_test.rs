#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_object_binding_pattern16() {
    let mut t = TestingT;
    run_test_completion_list_in_object_binding_pattern16(&mut t);
}

fn run_test_completion_list_in_object_binding_pattern16(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInObjectBindingPattern16") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @filename: a.js
/**
 * @typedef Foo
 * @property {number} a
 * @property {string} b
 */

/**
 * @param {Foo} options
 */
function f({ /**/ }) {}";
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
                exact: vec![
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
