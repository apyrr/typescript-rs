#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_object_binding_pattern15() {
    let mut t = TestingT;
    run_test_completion_list_in_object_binding_pattern15(&mut t);
}

fn run_test_completion_list_in_object_binding_pattern15(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInObjectBindingPattern15") {
        return;
    }
    let content = r"class Foo {
    private   xxx1 = 1;
    protected xxx2 = 2;
    public    xxx3 = 3;
    private   static xxx4 = 4;
    protected static xxx5 = 5;
    public    static xxx6 = 6;
    foo() {
        const { /*1*/ } = this;
        const { /*2*/ } = Foo;
    }
}

const { /*3*/ } = new Foo();
const { /*4*/ } = Foo;";
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
                    CompletionsExpectedItem::Label("xxx1".to_string()),
                    CompletionsExpectedItem::Label("xxx2".to_string()),
                    CompletionsExpectedItem::Label("xxx3".to_string()),
                    CompletionsExpectedItem::Label("foo".to_string()),
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
                    CompletionsExpectedItem::Label("prototype".to_string()),
                    CompletionsExpectedItem::Label("xxx4".to_string()),
                    CompletionsExpectedItem::Label("xxx5".to_string()),
                    CompletionsExpectedItem::Label("xxx6".to_string()),
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
                    CompletionsExpectedItem::Label("xxx3".to_string()),
                    CompletionsExpectedItem::Label("foo".to_string()),
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
                    CompletionsExpectedItem::Label("prototype".to_string()),
                    CompletionsExpectedItem::Label("xxx6".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
