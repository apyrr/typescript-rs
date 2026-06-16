#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_literal_directly_in_rest_constrained_to_array_type() {
    let mut t = TestingT;
    run_test_completions_literal_directly_in_rest_constrained_to_array_type(&mut t);
}

fn run_test_completions_literal_directly_in_rest_constrained_to_array_type(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsLiteralDirectlyInRestConstrainedToArrayType") {
        return;
    }
    let content = r"// @strict: true

function fn<T extends ('value1' | 'value2' | 'value3')[]>(...values: T): T { return values; }

const value1 = fn('/*1*/');
const value2 = fn('value1', '/*2*/');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "2".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("value1".to_string()),
                    CompletionsExpectedItem::Label("value2".to_string()),
                    CompletionsExpectedItem::Label("value3".to_string()),
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
