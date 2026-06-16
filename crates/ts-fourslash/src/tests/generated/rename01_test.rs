#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename01() {
    let mut t = TestingT;
    run_test_rename01(&mut t);
}

fn run_test_rename01(t: &mut TestingT) {
    if should_skip_if_failing("TestRename01") {
        return;
    }
    let content = r#"// @lib: es5
///<reference path="./Bar.ts" />
[|function [|{| "contextRangeIndex": 0 |}Bar|]() {
    // This is a reference to [|Bar|] in a comment.
    "this is a reference to [|Bar|] in a string"
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[1].clone().into()]);
    done();
}
