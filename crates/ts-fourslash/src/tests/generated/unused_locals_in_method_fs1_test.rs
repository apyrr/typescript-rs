#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_locals_in_method_fs1() {
    let mut t = TestingT;
    run_test_unused_locals_in_method_fs1(&mut t);
}

fn run_test_unused_locals_in_method_fs1(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedLocalsInMethodFS1") {
        return;
    }
    let content = r"// @noUnusedLocals: true
// @noUnusedParameters: true
class greeter {
    public function1() {
        [| var /*0*/x,/*1*/ y = 10; |]
        use(y);
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "var y = 10;", false, 0, 0);
    done();
}
