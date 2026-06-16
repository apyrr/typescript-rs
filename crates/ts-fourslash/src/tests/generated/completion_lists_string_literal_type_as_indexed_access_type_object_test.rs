#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_lists_string_literal_type_as_indexed_access_type_object() {
    let mut t = TestingT;
    run_test_completion_lists_string_literal_type_as_indexed_access_type_object(&mut t);
}

fn run_test_completion_lists_string_literal_type_as_indexed_access_type_object(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListsStringLiteralTypeAsIndexedAccessTypeObject") {
        return;
    }
    let content = r#"let firstCase: "a/*case_1*/"["foo"]
let secondCase: "b/*case_2*/"["bar"]
let thirdCase: "c/*case_3*/"["baz"]
let fourthCase: "en/*case_4*/"["qux"]
interface Foo {
 bar: string;
 qux: string;
}
let fifthCase: Foo["b/*case_5*/"]
let sixthCase: Foo["qu/*case_6*/"]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec![
            "case_1".to_string(),
            "case_2".to_string(),
            "case_3".to_string(),
            "case_4".to_string(),
        ]),
        None,
    );
    f.verify_completions(
        t,
        MarkerInput::Name("case_5".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "bar".to_string(),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("case_6".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "qux".to_string(),
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
