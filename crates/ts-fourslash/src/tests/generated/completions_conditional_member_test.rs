#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_conditional_member() {
    let mut t = TestingT;
    run_test_completions_conditional_member(&mut t);
}

fn run_test_completions_conditional_member(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsConditionalMember") {
        return;
    }
    let content = r"declare function f<T extends string>(
  p: { a: T extends 'foo' ? { x: string } : { y: string } }
): void;

f<'foo'>({ a: { /*1*/ } });
f<string>({ a: { /*2*/ } });";
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "x".to_string(),
                    ..Default::default()
                })],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "y".to_string(),
                    ..Default::default()
                })],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
