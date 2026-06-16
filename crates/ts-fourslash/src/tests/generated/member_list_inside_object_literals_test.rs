#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_inside_object_literals() {
    let mut t = TestingT;
    run_test_member_list_inside_object_literals(&mut t);
}

fn run_test_member_list_inside_object_literals(t: &mut TestingT) {
    if should_skip_if_failing("TestMemberListInsideObjectLiterals") {
        return;
    }
    let content = r"namespace ObjectLiterals {
    interface MyPoint {
        x1: number;
        y1: number;
    }

    var p1: MyPoint = {
        /*1*/
    };

    var p2: MyPoint = {
        x1: 5,
        /*2*/
    };

    var p3: MyPoint = {
        x1/*3*/:
    };

    var p4: MyPoint = {
        /*4*/y1
    };
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["1".to_string(), "3".to_string(), "4".to_string()]),
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
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "x1".to_string(),
                        detail: Some("(property) MyPoint.x1: number".to_string()),
                        ..Default::default()
                    }),
                    CompletionsExpectedItem::Item(lsproto::CompletionItem {
                        label: "y1".to_string(),
                        detail: Some("(property) MyPoint.y1: number".to_string()),
                        ..Default::default()
                    }),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["2".to_string()]),
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
                    label: "y1".to_string(),
                    detail: Some("(property) MyPoint.y1: number".to_string()),
                    ..Default::default()
                })],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
