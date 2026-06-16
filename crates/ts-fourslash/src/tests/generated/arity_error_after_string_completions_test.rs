#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_arity_error_after_string_completions() {
    let mut t = TestingT;
    run_test_arity_error_after_string_completions(&mut t);
}

fn run_test_arity_error_after_string_completions(t: &mut TestingT) {
    if should_skip_if_failing("TestArityErrorAfterStringCompletions") {
        return;
    }
    let content = r#"// @strict: true

interface Events {
  click: any;
  drag: any;
}

declare function addListener<K extends keyof Events>(type: K, listener: (ev: Events[K]) => any): void;

/*1*/addListener/*2*/("/*3*/")"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["3".to_string()]),
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
                    CompletionsExpectedItem::Label("click".to_string()),
                    CompletionsExpectedItem::Label("drag".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"));
    done();
}
