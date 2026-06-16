#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_string() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_string(&mut t);
}

fn run_test_code_fix_infer_from_usage_string(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
// @noImplicitAny: true
function foo([|p, a, b |]) {
    var x
    p.charAt(x)
    a.charAt(0)
    b.concat('hi')
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "p: string, a: string, b: string | any[]", false, 0, 0);
    done();
}
