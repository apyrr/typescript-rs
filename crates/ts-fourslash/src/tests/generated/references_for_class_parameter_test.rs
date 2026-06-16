#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_class_parameter() {
    let mut t = TestingT;
    run_test_references_for_class_parameter(&mut t);
}

fn run_test_references_for_class_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForClassParameter") {
        return;
    }
    let content = r"var p = 2;

class p { }

class foo {
    constructor (/*1*/public /*2*/p: any) {
    }

    public f(p) {
        this./*3*/p = p;
    }

}

var n = new foo(undefined);
n./*4*/p = null;";
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
