#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_member_to_interface() {
    let mut t = TestingT;
    run_test_add_member_to_interface(&mut t);
}

fn run_test_add_member_to_interface(t: &mut TestingT) {
    if should_skip_if_failing("TestAddMemberToInterface") {
        return;
    }
    let content = r"
namespace /*check*/Mod{
}

interface MyInterface {
    /*insert*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.verify_quick_info_at(t, "check", "namespace Mod", "");
    f.go_to_marker(t, "insert");
    f.insert(t, "x: number;\n");
    f.verify_quick_info_at(t, "check", "namespace Mod", "");
    done();
}
