#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_object_binding_element_property_name02() {
    let mut t = TestingT;
    run_test_quick_info_for_object_binding_element_property_name02(&mut t);
}

fn run_test_quick_info_for_object_binding_element_property_name02(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForObjectBindingElementPropertyName02") {
        return;
    }
    let content = r"interface I {
    property1: number;
    property2: string;
}

var foo: I;
var { /**/property1: {} } = foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(property) I.property1: number", "");
    done();
}
