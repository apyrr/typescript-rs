#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_jsdoc_tag() {
    let mut t = TestingT;
    run_test_completions_jsdoc_tag(&mut t);
}

fn run_test_completions_jsdoc_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsJsdocTag") {
        return;
    }
    let content = r"/**
 * @typedef {object} T
 * /**/
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "@property".to_string(),
                    detail: Some("@property".to_string()),
                    kind: Some(lsproto::CompletionItemKind::KEYWORD),
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
