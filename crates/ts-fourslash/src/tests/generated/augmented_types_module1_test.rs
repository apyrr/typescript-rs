#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_augmented_types_module1() {
    let mut t = TestingT;
    run_test_augmented_types_module1(&mut t);
}

fn run_test_augmented_types_module1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace m1c {
    export interface I { foo(): void; }
}
var m1c = 1; // Should be allowed
var x: m1c./*1*/;
var /*2*/r = m1c;";
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
                exact: vec![CompletionsExpectedItem::Label("I".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_quick_info_at(t, "2", "var r: number", "");
    done();
}
