#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_property_access() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_property_access(&mut t);
}

fn run_test_code_fix_infer_from_usage_property_access(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitAny: true
function foo([|a, m, x |]) {
    a.b.c;

    var numeric = 0;
    numeric = m.n();

    x.y.z
    x.y.z.push(0);
    return x.y.z
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "a: { b: { c: any; }; }, m: { n: () => number; }, x: { y: { z: number[]; }; }",
        false,
        0,
        0,
    );
    done();
}
