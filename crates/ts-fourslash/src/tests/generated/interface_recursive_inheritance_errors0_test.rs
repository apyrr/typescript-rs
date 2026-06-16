#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_interface_recursive_inheritance_errors0() {
    let mut t = TestingT;
    run_test_interface_recursive_inheritance_errors0(&mut t);
}

fn run_test_interface_recursive_inheritance_errors0(t: &mut TestingT) {
    if should_skip_if_failing("TestInterfaceRecursiveInheritanceErrors0") {
        return;
    }
    let content = r"interface i8 extends i9 { }
interface i9 /*1*/{ }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.insert(t, "extends i8 ");
    done();
}
