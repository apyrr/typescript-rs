#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_recommended_union() {
    let mut t = TestingT;
    run_test_completions_recommended_union(&mut t);
}

fn run_test_completions_recommended_union(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsRecommended_union") {
        return;
    }
    let content = r#"// @strictNullChecks: true
const enum E { A = "A", B = "B" }
const enum E2 { X = "X", Y = "Y" }
const e: E | undefined = /*a*/
const e2: E | E2 = /*b*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("a".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "E".to_string(),
                    preselect: Some(true),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("b".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "E".to_string(),
                        preselect: Some(true),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "E2".to_string(),
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
    done();
}
