#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_basic_class_members() {
    let mut t = TestingT;
    run_test_basic_class_members(&mut t);
}

fn run_test_basic_class_members(t: &mut TestingT) {
    if should_skip_if_failing("TestBasicClassMembers") {
        return;
    }
    let content = r"class n {
    constructor (public x: number, public y: number, private z: string) { }
}
var t = new n(0, 1, '');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_eof(t);
    f.insert(t, "t.");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![
                    CompletionsExpectedItem::Label("x".to_string()),
                    CompletionsExpectedItem::Label("y".to_string()),
                ],
                excludes: vec!["z".to_string()],
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
