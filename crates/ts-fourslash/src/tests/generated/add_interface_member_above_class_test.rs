#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_interface_member_above_class() {
    let mut t = TestingT;
    run_test_add_interface_member_above_class(&mut t);
}

fn run_test_add_interface_member_above_class(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"
interface Intersection {
    /*insertHere*/
}
interface Scene { }
class /*className*/Sphere {
    constructor() {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "className", "class Sphere", "");
    f.go_to_marker(t, "insertHere");
    f.insert(t, "ray: Ray;");
    f.verify_quick_info_at(t, "className", "class Sphere", "");
    done();
}
