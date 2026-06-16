#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_in_function_type_reference() {
    let mut t = TestingT;
    run_test_quick_info_in_function_type_reference(&mut t);
}

fn run_test_quick_info_in_function_type_reference(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function map(fn: (variab/*1*/le1: string) => void) {
}
var x = <{ (fn: (va/*2*/riable2: string) => void, a: string): void; }> () => { };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) variable1: string", "");
    f.verify_quick_info_at(t, "2", "(parameter) variable2: string", "");
    done();
}
