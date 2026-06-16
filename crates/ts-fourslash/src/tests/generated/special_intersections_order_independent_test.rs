#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_special_intersections_order_independent() {
    let mut t = TestingT;
    run_test_special_intersections_order_independent(&mut t);
}

fn run_test_special_intersections_order_independent(t: &mut TestingT) {
    if should_skip_if_failing("TestSpecialIntersectionsOrderIndependent") {
        return;
    }
    let content = r"declare function a(arg: 'test' | (string & {})): void
a('/*1*/')
declare function b(arg: 'test' | ({} & string)): void
b('/*2*/')";
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
                exact: vec![CompletionsExpectedItem::Label("test".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
