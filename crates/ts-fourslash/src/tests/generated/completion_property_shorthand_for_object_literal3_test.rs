#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_property_shorthand_for_object_literal3() {
    let mut t = TestingT;
    run_test_completion_property_shorthand_for_object_literal3(&mut t);
}

fn run_test_completion_property_shorthand_for_object_literal3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
const foo = 1;
const bar = 2;
const obj = {
  foo b/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals_plus(
                    vec![
                        CompletionsExpectedItem::Label("bar".to_string()),
                        CompletionsExpectedItem::Label("foo".to_string()),
                    ],
                    false,
                ),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
