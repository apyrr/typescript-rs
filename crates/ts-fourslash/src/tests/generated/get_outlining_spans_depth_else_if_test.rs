#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_spans_depth_else_if() {
    let mut t = TestingT;
    run_test_get_outlining_spans_depth_else_if(&mut t);
}

fn run_test_get_outlining_spans_depth_else_if(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningSpansDepthElseIf") {
        return;
    }
    let content = r"if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else if (1)[| {
    1;
}|] else[| {
    1;
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
