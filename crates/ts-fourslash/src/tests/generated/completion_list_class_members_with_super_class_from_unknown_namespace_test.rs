#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_class_members_with_super_class_from_unknown_namespace() {
    let mut t = TestingT;
    run_test_completion_list_class_members_with_super_class_from_unknown_namespace(&mut t);
}

fn run_test_completion_list_class_members_with_super_class_from_unknown_namespace(
    t: &mut TestingT,
) {
    if should_skip_if_failing("TestCompletionListClassMembersWithSuperClassFromUnknownNamespace") {
        return;
    }
    let content = r"class Child extends Namespace.Parent {
    /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: completion_class_element_keywords(),
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
