#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_multiple_parameters() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_multiple_parameters(&mut t);
}

fn run_test_code_fix_infer_from_usage_multiple_parameters(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noImplicitAny: true
function f([|a, b, c, d: number, e = 0, ...d |]) {
}
f(1, "string", { a: 1 }, {shouldNotBeHere: 2}, {shouldNotBeHere: 2}, 3, "string");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "a: number, b: string, c: { a: number; }, d: number, e = 0, ...d: (string | number)[]",
        false,
        0,
        1,
    );
    done();
}
