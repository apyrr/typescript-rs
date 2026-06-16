#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_on_function_parameter() {
    let mut t = TestingT;
    run_test_member_list_on_function_parameter(&mut t);
}

fn run_test_member_list_on_function_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberListOnFunctionParameter") {
        return;
    }
    let content = r"namespace Test10 {
    var x: string[] = [];
    x.forEach(function (y) { y./**/} );
}";
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
                includes: vec![CompletionsExpectedItem::Label("charAt".to_string())],
                excludes: vec!["toFixed".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
