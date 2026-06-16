#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_property_from_parent_constructor_function() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_property_from_parent_constructor_function(&mut t);
}

fn run_test_code_fix_class_implement_interface_property_from_parent_constructor_function(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"class A {
    constructor(public x: number) { }
}

class B implements A {[| |]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
