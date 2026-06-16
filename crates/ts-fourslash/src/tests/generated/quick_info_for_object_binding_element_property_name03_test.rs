#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_object_binding_element_property_name03() {
    let mut t = TestingT;
    run_test_quick_info_for_object_binding_element_property_name03(&mut t);
}

fn run_test_quick_info_for_object_binding_element_property_name03(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForObjectBindingElementPropertyName03") {
        return;
    }
    let content = r"// @strict: false
interface Recursive {
    next?: Recursive;
    value: any;
}

function f ({ /*1*/next: { /*2*/next: x} }: Recursive) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.verify_quick_info_at(t, &marker, "(property) Recursive.next?: Recursive", "");
    }
    done();
}
