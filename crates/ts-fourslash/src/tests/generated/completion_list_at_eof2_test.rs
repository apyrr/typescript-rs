#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_eof2() {
    let mut t = TestingT;
    run_test_completion_list_at_eof2(&mut t);
}

fn run_test_completion_list_at_eof2(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAtEOF2") {
        return;
    }
    let content = r"namespace Shapes {
    export class Point {
        constructor(public x: number, public y: number) { }
    }
}
var p = <Shapes.";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_eof(t);
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
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![CompletionsExpectedItem::Label("Point".to_string())],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
