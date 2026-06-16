#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_dot_dot_dot_in_object_literal1() {
    let mut t = TestingT;
    run_test_completions_dot_dot_dot_in_object_literal1(&mut t);
}

fn run_test_completions_dot_dot_dot_in_object_literal1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// https://github.com/microsoft/TypeScript/issues/57540

const foo = { b: 100 };

const bar: {
  a: number;
  b: number;
} = {
  a: 42,
  .../*1*/
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("foo".to_string())],
                excludes: vec!["b".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
