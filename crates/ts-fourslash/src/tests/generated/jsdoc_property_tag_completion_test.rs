#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_property_tag_completion() {
    let mut t = TestingT;
    run_test_jsdoc_property_tag_completion(&mut t);
}

fn run_test_jsdoc_property_tag_completion(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocPropertyTagCompletion") {
        return;
    }
    let content = r"// @lib: es5
/**
 * @typedef {Object} Foo
 * @property {/**/}
 */";
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
                exact: completion_global_types(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
