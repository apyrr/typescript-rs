#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_after_numeric_literal1() {
    let mut t = TestingT;
    run_test_completion_list_after_numeric_literal1(&mut t);
}

fn run_test_completion_list_after_numeric_literal1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAfterNumericLiteral1") {
        return;
    }
    let content = r"5../**/";
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("toExponential".to_string()),
                    CompletionsExpectedItem::Label("toFixed".to_string()),
                    CompletionsExpectedItem::Label("toLocaleString".to_string()),
                    CompletionsExpectedItem::Label("toPrecision".to_string()),
                    CompletionsExpectedItem::Label("toString".to_string()),
                    CompletionsExpectedItem::Label("valueOf".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
