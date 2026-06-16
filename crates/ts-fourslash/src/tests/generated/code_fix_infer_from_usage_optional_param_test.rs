#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_optional_param() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_optional_param(&mut t);
}

fn run_test_code_fix_infer_from_usage_optional_param(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixInferFromUsageOptionalParam") {
        return;
    }
    let content = r"// @strict: false
// @noImplicitAny: true
function f([|a? |]){
    a;
}
f();
f(1);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "a?: number", false, 0, 0);
    done();
}
