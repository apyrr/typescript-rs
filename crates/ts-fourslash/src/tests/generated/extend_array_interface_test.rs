#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_extend_array_interface() {
    let mut t = TestingT;
    run_test_extend_array_interface(&mut t);
}

fn run_test_extend_array_interface(t: &mut TestingT) {
    if should_skip_if_failing("TestExtendArrayInterface") {
        return;
    }
    let content = r"var x = [1, 2, 3];
x./*1*/concat([4]);
x./*2*/foo/*3*/()./*6*/toExponential/*7*/(2);
x./*4*/foo/*5*/()./*8*/charAt/*9*/(0);
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
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
                includes: vec![CompletionsExpectedItem::Label("concat".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_error_exists_between_markers(&f.marker_by_name("2"), &f.marker_by_name("3"));
    f.verify_error_exists_between_markers(&f.marker_by_name("4"), &f.marker_by_name("5"));
    f.go_to_eof(t);
    f.insert_line(t, "interface Array<T> { foo(): T; }");
    f.verify_no_error_exists_between_markers(&f.marker_by_name("2"), &f.marker_by_name("3"));
    f.verify_no_error_exists_between_markers(&f.marker_by_name("4"), &f.marker_by_name("5"));
    f.verify_no_error_exists_between_markers(&f.marker_by_name("6"), &f.marker_by_name("7"));
    f.verify_error_exists_between_markers(&f.marker_by_name("8"), &f.marker_by_name("9"));
    f.verify_number_of_errors_in_current_file(1);
    done();
}
