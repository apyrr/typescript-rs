#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_with_negative_number() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_with_negative_number(&mut t);
}

fn run_test_code_fix_class_implement_interface_with_negative_number(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface X { value: -1 | 0 | 1; }
class Y implements X { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(t, Some(&vec!["Implement interface 'X'".to_string()]));
    done();
}
