#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_recommended_equals() {
    let mut t = TestingT;
    run_test_completions_recommended_equals(&mut t);
}

fn run_test_completions_recommended_equals(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"enum Enu {}
declare const e: Enu;
e === /*a*/;
e === E/*b*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["a".to_string(), "b".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "Enu".to_string(),
                    detail: Some("enum Enu".to_string()),
                    kind: Some(lsproto::CompletionItemKind::ENUM),
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
    done();
}
