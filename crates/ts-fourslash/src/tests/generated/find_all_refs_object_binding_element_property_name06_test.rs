#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_object_binding_element_property_name06() {
    let mut t = TestingT;
    run_test_find_all_refs_object_binding_element_property_name06(&mut t);
}

fn run_test_find_all_refs_object_binding_element_property_name06(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsObjectBindingElementPropertyName06") {
        return;
    }
    let content = r"interface I {
    /*0*/property1: number;
    property2: string;
}

var elems: I[];
for (let { /*1*/property1: p } of elems) {
}
for (let { /*2*/property1 } of elems) {
}
for (var { /*3*/property1: p1 } of elems) {
}
var p2;
for ({ /*4*/property1 : p2 } of elems) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "3".to_string(),
            "4".to_string(),
            "2".to_string(),
        ],
    );
    done();
}
