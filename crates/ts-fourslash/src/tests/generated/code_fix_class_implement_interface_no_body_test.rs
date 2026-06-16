#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_no_body() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_no_body(&mut t);
}

fn run_test_code_fix_class_implement_interface_no_body(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
   m(): void
}
class C/*c*/ implements I";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_error_exists_before_marker(&f.marker_by_name("c"), 0);
    f.go_to_marker(t, "c");
    f.verify_code_fix_available(t, Some(&vec!["Implement interface 'I'".to_string()]));
    done();
}
