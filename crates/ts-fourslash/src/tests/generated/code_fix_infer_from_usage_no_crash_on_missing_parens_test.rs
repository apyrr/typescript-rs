#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_no_crash_on_missing_parens() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_no_crash_on_missing_parens(&mut t);
}

fn run_test_code_fix_infer_from_usage_no_crash_on_missing_parens(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitAny: true
// @target: esnext
class C {
    m() { this.x * 2; }
    get x { return null; }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
