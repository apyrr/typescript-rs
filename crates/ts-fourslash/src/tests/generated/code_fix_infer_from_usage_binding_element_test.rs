#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_binding_element() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_binding_element(&mut t);
}

fn run_test_code_fix_infer_from_usage_binding_element(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function f([car, cdr]) {
    return car + cdr + 1
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
