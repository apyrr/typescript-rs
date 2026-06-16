#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_class_interface_insert() {
    let mut t = TestingT;
    run_test_class_interface_insert(&mut t);
}

fn run_test_class_interface_insert(t: &mut TestingT) {
    if should_skip_if_failing("TestClassInterfaceInsert") {
        return;
    }
    let content = r"interface Intersection {
    dist: number;
}
/*interfaceGoesHere*/
class /*className*/Sphere {
    constructor(private center) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "className", "class Sphere", "");
    f.go_to_marker(t, "interfaceGoesHere");
    f.insert(t, "\ninterface Surface {\n    reflect: () => number;\n}\n");
    f.verify_quick_info_at(t, "className", "class Sphere", "");
    done();
}
