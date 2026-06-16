#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_entry_for_property_constrained_to_string() {
    let mut t = TestingT;
    run_test_completion_entry_for_property_constrained_to_string(&mut t);
}

fn run_test_completion_entry_for_property_constrained_to_string(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionEntryForPropertyConstrainedToString") {
        return;
    }
    let content = r#"declare function test<P extends "a" | "b">(p: { type: P }): void;

test({ type: /*ts*/ })"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["ts".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("\"a\"".to_string()),
                    CompletionsExpectedItem::Label("\"b\"".to_string()),
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
