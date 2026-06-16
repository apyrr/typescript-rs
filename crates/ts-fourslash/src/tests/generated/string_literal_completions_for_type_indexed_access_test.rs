#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_literal_completions_for_type_indexed_access() {
    let mut t = TestingT;
    run_test_string_literal_completions_for_type_indexed_access(&mut t);
}

fn run_test_string_literal_completions_for_type_indexed_access(t: &mut TestingT) {
    if should_skip_if_failing("TestStringLiteralCompletionsForTypeIndexedAccess") {
        return;
    }
    let content = r#"type Foo = { a: string; b: number; c: boolean; };
type A = Foo["/*1*/"];
type AorB = Foo["a" | "/*2*/"];"#;
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
                exact: vec![
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("c".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["2".to_string()]),
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
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("c".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
