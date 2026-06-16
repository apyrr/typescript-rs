#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_of_split_interface() {
    let mut t = TestingT;
    run_test_completion_list_of_split_interface(&mut t);
}

fn run_test_completion_list_of_split_interface(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListOfSplitInterface") {
        return;
    }
    let content = r"interface A {
    a: number;
}
interface I extends A {
    i1: number;
}
interface I1 extends A {
    i11: number;
}
interface B {
    b: number;
}
interface B1 {
    b1: number;
}
interface I extends B {
    i2: number;
}
interface I1 extends B, B1 {
    i12: number;
}
interface C {
    c: number;
}
interface I extends C {
    i3: number;
}
var ci: I;
ci./*1*/b;
var ci1: I1;
ci1./*2*/b;";
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("i1".to_string()),
                    CompletionsExpectedItem::Label("i2".to_string()),
                    CompletionsExpectedItem::Label("i3".to_string()),
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("c".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
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
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: vec![
                    CompletionsExpectedItem::Label("i11".to_string()),
                    CompletionsExpectedItem::Label("i12".to_string()),
                    CompletionsExpectedItem::Label("a".to_string()),
                    CompletionsExpectedItem::Label("b".to_string()),
                    CompletionsExpectedItem::Label("b1".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
