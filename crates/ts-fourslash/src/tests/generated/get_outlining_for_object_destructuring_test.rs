#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_for_object_destructuring() {
    let mut t = TestingT;
    run_test_get_outlining_for_object_destructuring(&mut t);
}

fn run_test_get_outlining_for_object_destructuring(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningForObjectDestructuring") {
        return;
    }
    let content = r"const[| {
    a,
    b,
    c
}|] =[| {
    a: 1,
    b: 2,
    c: 3
}|]
const[| {
    a:[| {
        a_1,
        a_2,
        a_3:[| {
            a_3_1,
            a_3_2,
            a_3_3,
        }|],
    }|],
    b,
    c
}|] =[| {
    a:[| {
        a_1: 1,
        a_2: 2,
        a_3:[| {
            a_3_1: 1,
            a_3_2: 1,
            a_3_3: 1
        }|],
    }|],
    b: 2,
    c: 3
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
