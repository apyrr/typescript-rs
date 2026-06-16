#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_module_members_of_generic_type() {
    let mut t = TestingT;
    run_test_module_members_of_generic_type(&mut t);
}

fn run_test_module_members_of_generic_type(t: &mut TestingT) {
    if should_skip_if_failing("TestModuleMembersOfGenericType") {
        return;
    }
    let content = r"namespace M {
    export var x = <T>(x: T) => x;
}
var r = M./**/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "x".to_string(),
                    detail: Some("var M.x: <T>(x: T) => T".to_string()),
                    ..Default::default()
                })],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
