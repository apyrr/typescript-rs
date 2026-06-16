#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_for_rest() {
    let mut t = TestingT;
    run_test_completion_list_for_rest(&mut t);
}

fn run_test_completion_list_for_rest(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListForRest") {
        return;
    }
    let content = r"interface Gen {
    x: number;
    parent: Gen;
    millenial: string;
}
let t: Gen;
var { x, ...rest } = t;
rest./*1*/x;";
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
                exact: vec![
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "millenial".to_string(),
                        detail: Some("(property) Gen.millenial: string".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "parent".to_string(),
                        detail: Some("(property) Gen.parent: Gen".to_string()),
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
