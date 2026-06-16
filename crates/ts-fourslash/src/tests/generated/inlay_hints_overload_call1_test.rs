#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_overload_call1() {
    let mut t = TestingT;
    run_test_inlay_hints_overload_call1(&mut t);
}

fn run_test_inlay_hints_overload_call1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Call {
    (a: number): void
    (b: number, c: number): void
    new (d: number): Call
}
declare const call: Call;
call(1);
call(1, 2);
new call(1);
declare function foo(w: number): void
declare function foo(a: number, b: number): void;
declare function foo(a: number | undefined, b: number | undefined): void;
foo(1)
foo(1, 2)
class Class {
    constructor(a: number);
    constructor(b: number, c: number);
    constructor(b: number, c?: number) { }
}
new Class(1)
new Class(1, 2)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
