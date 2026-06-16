#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_object_binding_element_property_name03() {
    let mut t = TestingT;
    run_test_find_all_refs_object_binding_element_property_name03(&mut t);
}

fn run_test_find_all_refs_object_binding_element_property_name03(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    /*1*/property1: number;
    property2: string;
}

var foo: I;
var [ { property1: prop1 }, { /*2*/property1, property2 } ] = [foo, foo];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
