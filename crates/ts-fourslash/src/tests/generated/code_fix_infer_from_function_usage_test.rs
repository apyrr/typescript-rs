#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_function_usage() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_function_usage(&mut t);
}

fn run_test_code_fix_infer_from_function_usage(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @stableTypeOrdering: true
// @noImplicitAny: true
function wrap( [| arr |] ) {
     arr.other(function (a: number, b: number) { return a < b ? -1 : 1 });
 }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "arr: { other: (arg0: (a: number, b: number) => -1 | 1) => void; }",
        false,
        0,
        0,
    );
    done();
}
