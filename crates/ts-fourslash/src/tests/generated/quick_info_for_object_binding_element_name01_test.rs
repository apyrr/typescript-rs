#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_object_binding_element_name01() {
    let mut t = TestingT;
    run_test_quick_info_for_object_binding_element_name01(&mut t);
}

fn run_test_quick_info_for_object_binding_element_name01(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForObjectBindingElementName01") {
        return;
    }
    let content = r"interface I {
    property1: number;
    property2: string;
}

var foo: I;
var { /**/property1 } = foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_at(t, "", "var property1: number", "");
    done();
}
