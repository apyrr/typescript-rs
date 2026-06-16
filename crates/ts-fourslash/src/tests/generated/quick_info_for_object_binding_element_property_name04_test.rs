#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_object_binding_element_property_name04() {
    let mut t = TestingT;
    run_test_quick_info_for_object_binding_element_property_name04(&mut t);
}

fn run_test_quick_info_for_object_binding_element_property_name04(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForObjectBindingElementPropertyName04") {
        return;
    }
    let content = r"interface Recursive {
    next?: Recursive;
    value: any;
}

function f ({ /*1*/next: { /*2*/next: x} }) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) next: {\n    next: any;\n}", "");
    f.verify_quick_info_at(t, "2", "(property) next: any", "");
    done();
}
