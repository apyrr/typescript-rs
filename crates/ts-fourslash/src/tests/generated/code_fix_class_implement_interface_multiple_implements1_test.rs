#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_multiple_implements1() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_multiple_implements1(&mut t);
}

fn run_test_code_fix_class_implement_interface_multiple_implements1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
interface I1 {
    x: number;
}
interface I2 {
    y: number;
}

class C implements I1,I2 {[|
    |]y: number;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "\nx: number;\n", false, 0, 0);
    f.verify_code_fix_not_available(t, &[]);
    done();
}
