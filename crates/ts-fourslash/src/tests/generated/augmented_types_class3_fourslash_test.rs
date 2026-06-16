#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_augmented_types_class3_fourslash() {
    let mut t = TestingT;
    run_test_augmented_types_class3_fourslash(&mut t);
}

fn run_test_augmented_types_class3_fourslash(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class c/*1*/5b { public foo() { } }
namespace c/*2*/5b { export var y = 2; } // should be ok
/*3*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "class c5b\nnamespace c5b", "");
    f.verify_quick_info_at(t, "2", "class c5b\nnamespace c5b", "");
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
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "c5b".to_string(),
                    detail: Some("class c5b\nnamespace c5b".to_string()),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
