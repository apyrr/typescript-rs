#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rest_params_contextually_typed() {
    let mut t = TestingT;
    run_test_rest_params_contextually_typed(&mut t);
}

fn run_test_rest_params_contextually_typed(t: &mut TestingT) {
    if should_skip_if_failing("TestRestParamsContextuallyTyped") {
        return;
    }
    let content = r"var foo: Function = function (/*1*/a, /*2*/b, /*3*/c) { };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) a: any", "");
    f.verify_quick_info_at(t, "2", "(parameter) b: any", "");
    f.verify_quick_info_at(t, "3", "(parameter) c: any", "");
    done();
}
