#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_object_binding_element_property_name05() {
    let mut t = TestingT;
    run_test_find_all_refs_object_binding_element_property_name05(&mut t);
}

fn run_test_find_all_refs_object_binding_element_property_name05(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    property1: number;
    property2: string;
}

function f({ /**/property1: p }, { property1 }) {
    let x = property1;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
