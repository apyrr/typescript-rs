#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_class_local() {
    let mut t = TestingT;
    run_test_references_for_class_local(&mut t);
}

fn run_test_references_for_class_local(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForClassLocal") {
        return;
    }
    let content = r"var n = 14;

class foo {
    /*1*/private /*2*/n = 0;

    public bar() {
        this./*3*/n = 9;
    }

    constructor() {
        this./*4*/n = 4;
    }

    public bar2() {
        var n = 12;
    }
}";
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
