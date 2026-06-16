#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_index_property3() {
    let mut t = TestingT;
    run_test_references_for_index_property3(&mut t);
}

fn run_test_references_for_index_property3(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForIndexProperty3") {
        return;
    }
    let content = r#"interface Object {
    /*1*/toMyString();
}

var y: Object;
y./*2*/toMyString();

var x = {};
x["/*3*/toMyString"]();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
