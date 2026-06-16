#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_literals() {
    let mut t = TestingT;
    run_test_completions_literals(&mut t);
}

fn run_test_completions_literals(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsLiterals") {
        return;
    }
    let content = r#"const x: 0 | "one" = /**/;
const y: 0 | "one" | 1n = /*1*/;
const y2: 0 | "one" | 1n = 'one'/*2*/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "0".to_string(),
                        kind: Some(lsproto::CompletionItemKind::CONSTANT),
                        detail: Some("0".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "\"one\"".to_string(),
                        kind: Some(lsproto::CompletionItemKind::CONSTANT),
                        detail: Some("\"one\"".to_string()),
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
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "0".to_string(),
                        kind: Some(lsproto::CompletionItemKind::CONSTANT),
                        detail: Some("0".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "\"one\"".to_string(),
                        kind: Some(lsproto::CompletionItemKind::CONSTANT),
                        detail: Some("\"one\"".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "1n".to_string(),
                        kind: Some(lsproto::CompletionItemKind::CONSTANT),
                        detail: Some("1n".to_string()),
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
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec!["\"one\"".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
