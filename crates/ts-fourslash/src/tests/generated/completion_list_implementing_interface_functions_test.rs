#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_implementing_interface_functions() {
    let mut t = TestingT;
    run_test_completion_list_implementing_interface_functions(&mut t);
}

fn run_test_completion_list_implementing_interface_functions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I1 {
    a(): void;
    b(): void;
}

var imp1: I1 = {
    a() {},
    /*0*/
}

var imp2: I1 = {
    a: () => {},
    /*1*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["0".to_string(), "1".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("b".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
