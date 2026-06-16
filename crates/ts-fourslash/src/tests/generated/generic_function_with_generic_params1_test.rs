#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_function_with_generic_params1() {
    let mut t = TestingT;
    run_test_generic_function_with_generic_params1(&mut t);
}

fn run_test_generic_function_with_generic_params1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericFunctionWithGenericParams1") {
        return;
    }
    let content = r"var obj = function f<T>(a: T) {
    var x/**/x: T;
    return a;
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(local var) xx: T", "");
    done();
}
