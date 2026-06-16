#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_for_type_literal() {
    let mut t = TestingT;
    run_test_get_outlining_for_type_literal(&mut t);
}

fn run_test_get_outlining_for_type_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningForTypeLiteral") {
        return;
    }
    let content = r"type A =[| {
    a: number;
}|]

type B =[| {
   a:[| {
       a1:[| {
           a2:[| {
               x: number;
               y: number;
           }|]
       }|]
   }|],
   b:[| {
       x: number;
   }|],
   c:[| {
       x: number;
   }|]
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
