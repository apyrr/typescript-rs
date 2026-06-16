#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_broken_class_error_recovery() {
    let mut t = TestingT;
    run_test_broken_class_error_recovery(&mut t);
}

fn run_test_broken_class_error_recovery(t: &mut TestingT) {
    if should_skip_if_failing("TestBrokenClassErrorRecovery") {
        return;
    }
    let content = r"class Foo {
    constructor() { var x = [1, 2, 3 }
}
/**/
var bar = new Foo();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_error_exists_after_marker_name("");
    done();
}
