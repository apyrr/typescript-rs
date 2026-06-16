#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_identifier_error_recovery() {
    let mut t = TestingT;
    run_test_identifier_error_recovery(&mut t);
}

fn run_test_identifier_error_recovery(t: &mut TestingT) {
    if should_skip_if_failing("TestIdentifierErrorRecovery") {
        return;
    }
    let content = r"var /*1*/export/*2*/;
var foo;
var /*3*/class/*4*/;
var bar;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"));
    f.verify_error_exists_between_markers(&f.marker_by_name("3"), &f.marker_by_name("4"));
    f.verify_number_of_errors_in_current_file(3);
    f.go_to_eof(t);
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
                includes: vec![
                    CompletionsExpectedItem::Label("foo".to_string()),
                    CompletionsExpectedItem::Label("bar".to_string()),
                ],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
