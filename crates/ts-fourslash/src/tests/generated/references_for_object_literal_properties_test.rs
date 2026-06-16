#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_object_literal_properties() {
    let mut t = TestingT;
    run_test_references_for_object_literal_properties(&mut t);
}

fn run_test_references_for_object_literal_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForObjectLiteralProperties") {
        return;
    }
    let content = r#"var x = { /*1*/add: 0, b: "string" };
x["/*2*/add"];
x./*3*/add;
var y = x;
y./*4*/add;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
