#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_object_binding_element_property_name04() {
    let mut t = TestingT;
    run_test_find_all_refs_object_binding_element_property_name04(&mut t);
}

fn run_test_find_all_refs_object_binding_element_property_name04(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    /*0*/property1: number;
    property2: string;
}

function f({ /*1*/property1: p1 }: I,
           { /*2*/property1 }: I,
           { property1: p2 }) {

    return /*3*/property1 + 1;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
        ],
    );
    done();
}
