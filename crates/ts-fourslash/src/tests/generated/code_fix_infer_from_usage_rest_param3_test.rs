#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_rest_param3() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_rest_param3(&mut t);
}

fn run_test_code_fix_infer_from_usage_rest_param3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitAny: true
function f(a: number, [|...rest |]){
    a;
    rest.push(22);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "...rest: number[]", false, 0, 0);
    done();
}
