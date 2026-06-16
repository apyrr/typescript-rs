#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_rest_param2() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_rest_param2(&mut t);
}

fn run_test_code_fix_infer_from_usage_rest_param2(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixInferFromUsageRestParam2") {
        return;
    }
    let content = r#"// @strict: false
// @noImplicitAny: true
function f(a: number, [|...rest |]){
    a; rest;
}
f(1);
f(2, "s1");
f(3, false, "s2");
f(4, "s1", "s2", false, "s4");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "...rest: (string | boolean)[]", false, 0, 0);
    done();
}
