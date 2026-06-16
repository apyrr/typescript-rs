#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_type_completions_no_crash() {
    let mut t = TestingT;
    run_test_js_doc_function_type_completions_no_crash(&mut t);
}

fn run_test_js_doc_function_type_completions_no_crash(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionTypeCompletionsNoCrash") {
        return;
    }
    let content = r#"// @lib: es5
/**
 * @returns {function/**/(): string}
 */
function updateCalendarEvent() {
  return "";
}"#;
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
                exact: completion_global_types(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
