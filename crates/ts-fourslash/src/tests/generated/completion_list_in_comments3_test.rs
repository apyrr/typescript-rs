#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_comments3() {
    let mut t = TestingT;
    run_test_completion_list_in_comments3(&mut t);
}

fn run_test_completion_list_in_comments3(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInComments3") {
        return;
    }
    let content = r#"// @lib: es5
 /*{| "name": "1" |}
 /*  {| "name": "2" |}
 /*  *{| "name": "3" |}
 /*  */{| "name": "4" |}
 {| "name": "5" |}/*  */
/* {| "name": "6" |}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "6".to_string(),
        ]),
        None,
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["4".to_string(), "5".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_globals(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
