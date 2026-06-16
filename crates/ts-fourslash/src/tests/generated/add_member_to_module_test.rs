#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_member_to_module() {
    let mut t = TestingT;
    run_test_add_member_to_module(&mut t);
}

fn run_test_add_member_to_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace A {
    /*var*/
}
module /*check*/A {
    var p;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "check");
    f.verify_quick_info_exists(t);
    f.go_to_marker(t, "var");
    f.insert(t, "var o;");
    f.go_to_marker(t, "check");
    f.verify_quick_info_exists(t);
    done();
}
