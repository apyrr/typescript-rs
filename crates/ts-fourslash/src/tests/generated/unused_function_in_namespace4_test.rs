#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_function_in_namespace4() {
    let mut t = TestingT;
    run_test_unused_function_in_namespace4(&mut t);
}

fn run_test_unused_function_in_namespace4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noUnusedLocals: true
// @noUnusedParameters:true
 [| namespace Validation {
    var function1 = function() {
    }
} |]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "namespace Validation {\n}", false, 0, 0);
    done();
}
