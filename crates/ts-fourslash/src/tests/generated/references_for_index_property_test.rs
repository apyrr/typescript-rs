#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_index_property() {
    let mut t = TestingT;
    run_test_references_for_index_property(&mut t);
}

fn run_test_references_for_index_property(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForIndexProperty") {
        return;
    }
    let content = r#"class Foo {
    /*1*/property: number;
    /*2*/method(): void { }
}

var f: Foo;
f["/*3*/property"];
f["/*4*/method"];"#;
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
