#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_protected_members() {
    let mut t = TestingT;
    run_test_completion_list_protected_members(&mut t);
}

fn run_test_completion_list_protected_members(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Base {
    protected y;
    constructor(protected x) {}
    method() { this./*1*/; }
}
class D1 extends Base {
    protected z;
    method1() { this./*2*/; }
}
class D2 extends Base {
    method2() { this./*3*/; }
}
class D3 extends D1 {
    method2() { this./*4*/; }
}
var b: Base;
f./*5*/";
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
                    CompletionsExpectedItem::Label("y".to_string()),
                    CompletionsExpectedItem::Label("x".to_string()),
                    CompletionsExpectedItem::Label("method".to_string()),
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
                    CompletionsExpectedItem::Label("z".to_string()),
                    CompletionsExpectedItem::Label("method1".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                    CompletionsExpectedItem::Label("x".to_string()),
                    CompletionsExpectedItem::Label("method".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("3".to_string()),
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
                    CompletionsExpectedItem::Label("method2".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                    CompletionsExpectedItem::Label("x".to_string()),
                    CompletionsExpectedItem::Label("method".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Name("4".to_string()),
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
                    CompletionsExpectedItem::Label("method2".to_string()),
                    CompletionsExpectedItem::Label("z".to_string()),
                    CompletionsExpectedItem::Label("method1".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                    CompletionsExpectedItem::Label("x".to_string()),
                    CompletionsExpectedItem::Label("method".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(t, MarkerInput::Name("5".to_string()), None);
    done();
}
