#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_duplicate_member1() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_duplicate_member1(&mut t);
}

fn run_test_code_fix_class_implement_interface_duplicate_member1(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceDuplicateMember1") {
        return;
    }
    let content = r"interface I1 {
    x: number;
}
interface I2 {
    x: number;
}

class C implements I1,I2 {[| |]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(
        t,
        Some(&vec![
            "Implement interface 'I1'".to_string(),
            "Implement interface 'I2'".to_string(),
        ]),
    );
    done();
}
