#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_merged_declarations1() {
    let mut t = TestingT;
    run_test_completions_merged_declarations1(&mut t);
}

fn run_test_completions_merged_declarations1(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsMergedDeclarations1") {
        return;
    }
    let content = r"// @lib: es5
interface Point {
    x: number;
    y: number;
}
function point(x: number, y: number): Point {
    return { x: x, y: y };
}
namespace point {
    export var origin = point(0, 0);
    export function equals(p1: Point, p2: Point) {
        return p1.x == p2.x && p1.y == p2.y;
    }
}
var p1 = /*1*/point(0, 0);
var p2 = point./*2*/origin;
var b = point./*3*/equals(p1, p2);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(Vec::new()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Label("point".to_string())],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    f.verify_completions(
        t,
        MarkerInput::Names(vec!["2".to_string(), "3".to_string()]),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: completion_function_members_with_prototype_plus(vec![
                    CompletionsExpectedItem::Label("equals".to_string()),
                    CompletionsExpectedItem::Label("origin".to_string()),
                ]),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
