#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_redeclared_property_in_derived_interface() {
    let mut t = TestingT;
    run_test_find_all_refs_redeclared_property_in_derived_interface(&mut t);
}

fn run_test_find_all_refs_redeclared_property_in_derived_interface(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsRedeclaredPropertyInDerivedInterface") {
        return;
    }
    let content = r"// @noLib: true
interface A {
    readonly /*0*/x: number | string;
}
interface B extends A {
    readonly /*1*/x: number;
}
const a: A = { /*2*/x: 0 };
const b: B = { /*3*/x: 0 };";
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
