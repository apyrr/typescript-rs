#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_undefined() {
    let mut t = TestingT;
    run_test_quick_info_on_undefined(&mut t);
}

fn run_test_quick_info_on_undefined(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo(a: string) {
}
foo(/*1*/undefined);
var x = {
    undefined: 10
};
x./*2*/undefined = 30;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var undefined", "");
    f.verify_quick_info_at(t, "2", "(property) undefined: number", "");
    done();
}
