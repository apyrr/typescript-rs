#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_parameter_names2() {
    let mut t = TestingT;
    run_test_code_fix_add_parameter_names2(&mut t);
}

fn run_test_code_fix_add_parameter_names2(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAddParameterNames2") {
        return;
    }
    let content = r"// @noImplicitAny: true
type Rest = ([|...number|]) => void;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "...arg0: number[]", false, 0, 0);
    done();
}
