#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_comments_and_strings1() {
    let mut t = TestingT;
    run_test_rename_comments_and_strings1(&mut t);
}

fn run_test_rename_comments_and_strings1(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameCommentsAndStrings1") {
        return;
    }
    let content = r#"///<reference path="./Bar.ts" />
[|function [|{| "contextRangeIndex": 0 |}Bar|]() {
    // This is a reference to Bar in a comment.
    "this is a reference to Bar in a string"
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "Bar");
    done();
}
