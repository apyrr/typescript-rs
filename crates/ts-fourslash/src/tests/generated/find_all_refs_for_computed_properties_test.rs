#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_computed_properties() {
    let mut t = TestingT;
    run_test_find_all_refs_for_computed_properties(&mut t);
}

fn run_test_find_all_refs_for_computed_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForComputedProperties") {
        return;
    }
    let content = r#"interface I {
    ["/*0*/prop1"]: () => void;
}

class C implements I {
    ["/*1*/prop1"]: any;
}

var x: I = {
    ["/*2*/prop1"]: function () { },
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    done();
}
