#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_function_members() {
    let mut t = TestingT;
    run_test_completion_list_function_members(&mut t);
}

fn run_test_completion_list_function_members(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
function fnc1() {
    var bar = 1;
    function foob(){ }
}

fnc1./**/";
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
                exact: completion_function_members_with_prototype(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
