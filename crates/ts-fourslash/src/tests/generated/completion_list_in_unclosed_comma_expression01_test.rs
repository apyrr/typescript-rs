#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_unclosed_comma_expression01() {
    let mut t = TestingT;
    run_test_completion_list_in_unclosed_comma_expression01(&mut t);
}

fn run_test_completion_list_in_unclosed_comma_expression01(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInUnclosedCommaExpression01") {
        return;
    }
    let content = r"// should NOT see a and b
foo((a, b) => a,/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec!["a".to_string(), "b".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
