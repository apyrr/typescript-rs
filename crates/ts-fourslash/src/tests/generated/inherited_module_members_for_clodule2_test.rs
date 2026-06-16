#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inherited_module_members_for_clodule2() {
    let mut t = TestingT;
    run_test_inherited_module_members_for_clodule2(&mut t);
}

fn run_test_inherited_module_members_for_clodule2(t: &mut TestingT) {
    if should_skip_if_failing("TestInheritedModuleMembersForClodule2") {
        return;
    }
    let content = r"// @strict: false
namespace M {
    export namespace A {
        var o;
    }
}
namespace M {
    export class A { a = 1;}
}
namespace M {
    export class A { /**/b }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    f.verify_number_of_errors_in_current_file(4);
    done();
}
