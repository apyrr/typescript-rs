#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_contextually_typed_argument() {
    let mut t = TestingT;
    run_test_completion_list_in_contextually_typed_argument(&mut t);
}

fn run_test_completion_list_in_contextually_typed_argument(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInContextuallyTypedArgument") {
        return;
    }
    let content = r"interface MyPoint {
    x1: number;
    y1: number;
}

function foo(a: (e: MyPoint) => string) { }
foo((e) => {
    e./*1*/
} );

class test {
    constructor(a: (e: MyPoint) => string) { }
}
var t = new test((e) => {
    e./*2*/
} );";
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("x1".to_string()),
                    CompletionsExpectedItem::Label("y1".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
