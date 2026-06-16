#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_with_namespace_inside_function() {
    let mut t = TestingT;
    run_test_completion_with_namespace_inside_function(&mut t);
}

fn run_test_completion_with_namespace_inside_function(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionWithNamespaceInsideFunction") {
        return;
    }
    let content = r"function f() {
    namespace n {
        interface I {
            x: number
        }
        /*1*/
    }
    /*2*/
}
/*3*/
function f2() {
    namespace n2 {
        class I2 {
            x: number
        }
        /*11*/
    }
    /*22*/
}
/*33*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "2".to_string(), "3".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "f".to_string(),
                    detail: Some("function f(): void".to_string()),
                    ..Default::default()
                })],
                excludes: vec!["n".to_string(), "I".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("11".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "f2".to_string(),
                        detail: Some("function f2(): void".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "n2".to_string(),
                        detail: Some("namespace n2".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "I2".to_string(),
                        detail: Some("class I2".to_string()),
                        ..Default::default()
                    }),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("22".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "f2".to_string(),
                        detail: Some("function f2(): void".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "n2".to_string(),
                        detail: Some("namespace n2".to_string()),
                        ..Default::default()
                    }),
                ],
                excludes: vec!["I2".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("33".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "f2".to_string(),
                    detail: Some("function f2(): void".to_string()),
                    ..Default::default()
                })],
                excludes: vec!["n2".to_string(), "I2".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
