#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_inside_target_typed_function() {
    let mut t = TestingT;
    run_test_completion_list_inside_target_typed_function(&mut t);
}

fn run_test_completion_list_inside_target_typed_function(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInsideTargetTypedFunction") {
        return;
    }
    let content = r"namespace Fix2 {
    interface iFace { (event: string); }
    var foo: iFace = function (elem) { /**/ }
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "elem".to_string(),
                    detail: Some("(parameter) elem: string".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
