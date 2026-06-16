#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_beginning_of_identifier_in_arrow_function01() {
    let mut t = TestingT;
    run_test_completion_list_at_beginning_of_identifier_in_arrow_function01(&mut t);
}

fn run_test_completion_list_at_beginning_of_identifier_in_arrow_function01(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAtBeginningOfIdentifierInArrowFunction01") {
        return;
    }
    let content = r"xyz => /*1*/x";
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
                includes: vec![CompletionsExpectedItem::Label("xyz".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
