#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_in_with_block() {
    let mut t = TestingT;
    run_test_member_list_in_with_block(&mut t);
}

fn run_test_member_list_in_with_block(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberListInWithBlock") {
        return;
    }
    let content = r"class c {
    static x: number;
    public foo() {
        with ({}) {
            function f() { }
            var d = this./*1*/foo;
            /*2*/
        }
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("1".to_string()), None);
    f.verify_completions(
        t,
        MarkerInput::Name("2".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: vec![
                    "foo".to_string(),
                    "f".to_string(),
                    "c".to_string(),
                    "d".to_string(),
                    "x".to_string(),
                    "Object".to_string(),
                ],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
