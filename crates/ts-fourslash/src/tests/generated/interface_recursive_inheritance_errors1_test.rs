#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_interface_recursive_inheritance_errors1() {
    let mut t = TestingT;
    run_test_interface_recursive_inheritance_errors1(&mut t);
}

fn run_test_interface_recursive_inheritance_errors1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface i8 extends i9 { }
interface i9 /*1*/extends i8{ }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.delete_at_caret(t, 11);
    done();
}
