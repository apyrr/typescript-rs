#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_for_single_line_comments() {
    let mut t = TestingT;
    run_test_get_outlining_for_single_line_comments(&mut t);
}

fn run_test_get_outlining_for_single_line_comments(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningForSingleLineComments") {
        return;
    }
    let content = r"[|// Single line comments at the start of the file
// line 2
// line 3
// line 4|]
module Sayings[| {

    [|/*
    */|]
    [|// A sequence of
    // single line|]
    [|/*
        and block
    */|]
    [|// comments
    //|]
    export class Sample[| {
    }|]
}|]

interface IFoo[| {
    [|// all consecutive single line comments should be in one block regardless of their number or empty lines/spaces inbetween

    // comment 2
    // comment 3

    //comment 4
    /// comment 5
    ///// comment 6

    //comment 7
    ///comment 8
    // comment 9
    // //comment 10




















    // // //comment 11
    // comment 12
    // comment 13
    // comment 14
    // comment 15

    // comment 16
    // comment 17
    // comment 18
    // comment 19
    // comment 20    
    // comment 21|]

    getDist(): number; // One single line comment should not be collapsed
}|]

// One single line comment should not be collapsed
class WithOneSingleLineComment[| {
}|]

function Foo()[| {
   [|// comment 1
     // comment 2|]
    this.method = function (param)[| {
    }|]

   [|// comment 1
     // comment 2|]
    function method(param)[| {
    }|]
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
