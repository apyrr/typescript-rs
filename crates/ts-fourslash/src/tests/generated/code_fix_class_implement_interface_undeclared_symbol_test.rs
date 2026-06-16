#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_undeclared_symbol() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_undeclared_symbol(&mut t);
}

fn run_test_code_fix_class_implement_interface_undeclared_symbol(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
   x: T;
}

class C implements I { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(t, Some(&vec!["Implement interface 'I'".to_string()]));
    done();
}
