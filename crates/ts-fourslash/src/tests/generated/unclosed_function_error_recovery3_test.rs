#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unclosed_function_error_recovery3() {
    let mut t = TestingT;
    run_test_unclosed_function_error_recovery3(&mut t);
}

fn run_test_unclosed_function_error_recovery3(t: &mut TestingT) {
    if should_skip_if_failing("TestUnclosedFunctionErrorRecovery3") {
        return;
    }
    let content = r"// @allowUnreachableCode: true
       class alpha { static beta() return 5; } }
 /**/  var gamma = alpha.beta() * 5;             ";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_error_exists_after_marker_name("");
    done();
}
