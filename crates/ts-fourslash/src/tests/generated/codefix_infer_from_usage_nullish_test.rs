#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_codefix_infer_from_usage_nullish() {
    let mut t = TestingT;
    run_test_codefix_infer_from_usage_nullish(&mut t);
}

fn run_test_codefix_infer_from_usage_nullish(t: &mut TestingT) {
    if should_skip_if_failing("TestCodefixInferFromUsageNullish") {
        return;
    }
    let content = r"// @strict: false
// @noImplicitAny: true
declare const a: string
function wat([|b |]) {
    b(a ?? 1);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "b: (arg0: string | number) => void", false, 0, 0);
    done();
}
