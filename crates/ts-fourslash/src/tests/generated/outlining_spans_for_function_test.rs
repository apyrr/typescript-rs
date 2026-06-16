#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_outlining_spans_for_function() {
    let mut t = TestingT;
    run_test_outlining_spans_for_function(&mut t);
}

fn run_test_outlining_spans_for_function(t: &mut TestingT) {
    if should_skip_if_failing("TestOutliningSpansForFunction") {
        return;
    }
    let content = r"[|(
    a: number,
    b: number
) => {
    return a + b;
}|];

(a: number, b: number) =>[| {
    return a + b;
}|]

const f1 = function[| (
    a: number
    b: number
) {
    return a + b;
}|]

const f2 = function (a: number, b: number)[| {
    return a + b;
}|]

function f3[| (
    a: number
    b: number
) {
    return a + b;
}|]

function f4(a: number, b: number)[| {
    return a + b;
}|]

class Foo[| {
    constructor[|(
        a: number,
        b: number
    ) {
        this.a = a;
        this.b = b;
    }|]

    m1[|(
        a: number,
        b: number
    ) {
        return a + b;
    }|]

    m1(a: number, b: number)[| {
        return a + b;
    }|]
}|]

declare function foo(props: any): void;
foo[|(
    a =>[| {

    }|]
)|]

foo[|(
    (a) =>[| {

    }|]
)|]

foo[|(
    (a, b, c) =>[| {

    }|]
)|]

foo[|([|
    (a,
     b,
     c) => {

    }|]
)|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
