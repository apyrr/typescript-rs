#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_call_body_both() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_call_body_both(&mut t);
}

fn run_test_code_fix_infer_from_usage_call_body_both(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    p = 2
}
var c = new C()
function f([|x, y |]) {
    if (y) {
        x = 1
    }
    return x
}
f(new C())";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "x: number | C, y: undefined", false, 0, 1);
    done();
}
