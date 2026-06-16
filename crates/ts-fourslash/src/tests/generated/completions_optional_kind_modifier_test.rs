#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_optional_kind_modifier() {
    let mut t = TestingT;
    run_test_completions_optional_kind_modifier(&mut t);
}

fn run_test_completions_optional_kind_modifier(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface A { a?: number; method?(): number; };
function f(x: A) {
x./*a*/;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("a".to_string()),
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
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "a?".to_string(),
                        insert_text: Some("a".to_string()),
                        filter_text: Some("a".to_string()),
                        kind: Some(lsproto::CompletionItemKind::FIELD),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "method?".to_string(),
                        insert_text: Some("method".to_string()),
                        filter_text: Some("method".to_string()),
                        kind: Some(lsproto::CompletionItemKind::METHOD),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
