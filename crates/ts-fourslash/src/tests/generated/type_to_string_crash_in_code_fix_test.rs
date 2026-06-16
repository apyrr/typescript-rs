#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_to_string_crash_in_code_fix() {
    let mut t = TestingT;
    run_test_type_to_string_crash_in_code_fix(&mut t);
}

fn run_test_type_to_string_crash_in_code_fix(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitAny: true
function f([|y |], z = { p: y[";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "y: { [x: string]: any; }", false, 0, 0);
    done();
}
