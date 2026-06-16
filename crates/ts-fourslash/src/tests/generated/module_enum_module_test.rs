#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_module_enum_module() {
    let mut t = TestingT;
    run_test_module_enum_module(&mut t);
}

fn run_test_module_enum_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace A {
    var o;
}
enum A {
    /**/c
}
namespace A {
    var p;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    done();
}
