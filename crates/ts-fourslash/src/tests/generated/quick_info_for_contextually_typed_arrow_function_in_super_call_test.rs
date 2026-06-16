#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_contextually_typed_arrow_function_in_super_call() {
    let mut t = TestingT;
    run_test_quick_info_for_contextually_typed_arrow_function_in_super_call(&mut t);
}

fn run_test_quick_info_for_contextually_typed_arrow_function_in_super_call(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class A<T1, T2> {
    constructor(private map: (value: T1) => T2) {

    }
}

class B extends A<number, string> {
    constructor() { super(va/*1*/lue => String(va/*2*/lue.toExpone/*3*/ntial())); }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) value: number", "");
    f.verify_quick_info_at(t, "2", "(parameter) value: number", "");
    f.verify_quick_info_at(
        t,
        "3",
        "(method) Number.toExponential(fractionDigits?: number): string",
        "Returns a string containing a number represented in exponential notation.",
    );
    done();
}
