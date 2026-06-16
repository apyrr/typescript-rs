#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_triple_slash_ref_path_completion_context() {
    let mut t = TestingT;
    run_test_triple_slash_ref_path_completion_context(&mut t);
}

fn run_test_triple_slash_ref_path_completion_context(t: &mut TestingT) {
    if should_skip_if_failing("TestTripleSlashRefPathCompletionContext") {
        return;
    }
    let content = r#"// @Filename: f.ts
/*f*/
// @Filename: test.ts
/// <reference path/*0*/=/*1*/"/*8*/
/// <reference path/*2*/=/*3*/"/*9*/"/*4*/ /*5*///*6*/>/*7*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
        ]),
        None,
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["8".to_string(), "9".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("f.ts".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
