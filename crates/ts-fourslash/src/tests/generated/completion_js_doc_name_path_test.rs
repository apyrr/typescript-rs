#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_js_doc_name_path() {
    let mut t = TestingT;
    run_test_completion_js_doc_name_path(&mut t);
}

fn run_test_completion_js_doc_name_path(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionJSDocNamePath") {
        return;
    }
    let content = r"// @noLib: true
/**
 * @returns {modu/*1*/le:ControlFlow}
 */
export function cargo() {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
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
                includes: Vec::new(),
                excludes: vec!["module".to_string(), "ControlFlow".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
