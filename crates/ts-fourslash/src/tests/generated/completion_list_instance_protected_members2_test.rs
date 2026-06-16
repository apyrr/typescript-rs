#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_instance_protected_members2() {
    let mut t = TestingT;
    run_test_completion_list_instance_protected_members2(&mut t);
}

fn run_test_completion_list_instance_protected_members2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Base {
    private privateMethod() { }
    private privateProperty;

    protected protectedMethod() { }
    protected protectedProperty;

    public publicMethod() { }
    public publicProperty;

    protected protectedOverriddenMethod() { }
    protected protectedOverriddenProperty;
}

class C1 extends Base {
    protected  protectedOverriddenMethod() { }
    protected  protectedOverriddenProperty;

    test() {
        this./*1*/;
        super./*2*/;

        var b: Base;
        var c: C1;

        b./*3*/;
        c./*4*/;
    }
}";
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
                includes: vec![
                    CompletionsExpectedItem::Label("protectedMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedProperty".to_string()),
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("publicProperty".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenProperty".to_string()),
                ],
                excludes: vec!["privateMethod".to_string(), "privateProperty".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
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
                includes: vec![
                    CompletionsExpectedItem::Label("protectedMethod".to_string()),
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenMethod".to_string()),
                ],
                excludes: vec![
                    "privateMethod".to_string(),
                    "privateProperty".to_string(),
                    "protectedProperty".to_string(),
                    "publicProperty".to_string(),
                    "protectedOverriddenProperty".to_string(),
                ],
                exact: Vec::new(),
                unsorted: Vec::new(),
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
                includes: vec![
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("publicProperty".to_string()),
                ],
                excludes: vec![
                    "privateMethod".to_string(),
                    "privateProperty".to_string(),
                    "protectedMethod".to_string(),
                    "protectedProperty".to_string(),
                    "protectedOverriddenMethod".to_string(),
                    "protectedOverriddenProperty".to_string(),
                ],
                exact: Vec::new(),
                unsorted: Vec::new(),
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
                includes: vec![
                    CompletionsExpectedItem::Label("protectedMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedProperty".to_string()),
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("publicProperty".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenProperty".to_string()),
                ],
                excludes: vec!["privateMethod".to_string(), "privateProperty".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
