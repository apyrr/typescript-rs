#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_localsin_constructor_fs1() {
    let mut t = TestingT;
    run_test_unused_localsin_constructor_fs1(&mut t);
}

fn run_test_unused_localsin_constructor_fs1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noUnusedLocals: true
// @noUnusedParameters:true
class greeter {
    [| constructor() {
        var unused = 20;
    } |]
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "constructor() {\n}", false, 0, 0);
    done();
}
