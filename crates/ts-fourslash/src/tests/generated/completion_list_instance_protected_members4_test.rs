#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_instance_protected_members4() {
    let mut t = TestingT;
    run_test_completion_list_instance_protected_members4(&mut t);
}

fn run_test_completion_list_instance_protected_members4(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListInstanceProtectedMembers4") {
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
}

class C1 extends Base {
    public protectedOverriddenMethod() { }
    public protectedOverriddenProperty;
}

 var c: C1;
 c./*1*/";
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
                exact: vec![
                    CompletionsExpectedItem::Label("protectedOverriddenMethod".to_string()),
                    CompletionsExpectedItem::Label("protectedOverriddenProperty".to_string()),
                    CompletionsExpectedItem::Label("publicMethod".to_string()),
                    CompletionsExpectedItem::Label("publicProperty".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
