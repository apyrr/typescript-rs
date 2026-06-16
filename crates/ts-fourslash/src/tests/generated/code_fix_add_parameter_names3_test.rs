#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_parameter_names3() {
    let mut t = TestingT;
    run_test_code_fix_add_parameter_names3(&mut t);
}

fn run_test_code_fix_add_parameter_names3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitAny: true
type Rest = ([|public string|]) => void;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "public arg0: string", false, 0, 0);
    done();
}
