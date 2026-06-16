#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_before_semantic_diagnostics_in_arrow_function1() {
    let mut t = TestingT;
    run_test_completion_before_semantic_diagnostics_in_arrow_function1(&mut t);
}

fn run_test_completion_before_semantic_diagnostics_in_arrow_function1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var f4 = <T>(x: T/**/ ) => {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.backspace(t, 1);
    f.insert(t, "A");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "T".to_string(),
                    detail: Some("(type parameter) T in <T>(x: A): void".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_number_of_errors_in_current_file(1);
    done();
}
