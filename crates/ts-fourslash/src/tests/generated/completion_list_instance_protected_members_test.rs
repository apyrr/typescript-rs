#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_instance_protected_members() {
    let mut t = TestingT;
    run_test_completion_list_instance_protected_members(&mut t);
}

fn run_test_completion_list_instance_protected_members(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInstanceProtectedMembers") {
        return;
    }
    let content = r"class Base {
    private privateMethod() { }
    private privateProperty;

    protected protectedMethod() { }
    protected protectedProperty;

    public publicMethod() { }
    public publicProperty;

    protected protectedOverriddenMethod() { }
    protected protectedOverriddenProperty;

    test() {
        this./*1*/;

        var b: Base;
        var c: C1;

        b./*2*/;
        c./*3*/;
    }
}

class C1 extends Base {
    protected  protectedOverriddenMethod() { }
    protected  protectedOverriddenProperty;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "2".to_string()]),
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
                    CompletionsExpectedItem::Label("privateMethod".to_string()),
                    CompletionsExpectedItem::Label("privateProperty".to_string()),
                    CompletionsExpectedItem::Label("protectedMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedProperty".to_string()),
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("publicProperty".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenProperty".to_string()),
                    CompletionsExpectedItem::Label("test".to_string()),
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
                    CompletionsExpectedItem::Label("privateMethod".to_string()),
                    CompletionsExpectedItem::Label("privateProperty".to_string()),
                    CompletionsExpectedItem::Label("protectedMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedProperty".to_string()),
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("publicProperty".to_string()),
                    CompletionsExpectedItem::Label("test".to_string()),
                ],
            }),
            user_preferences: None,
        }),
    );
    done();
}
