#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_nested_object_literals() {
    let mut t = TestingT;
    run_test_navigation_bar_nested_object_literals(&mut t);
}

fn run_test_navigation_bar_nested_object_literals(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarNestedObjectLiterals") {
        return;
    }
    let content = r"var a = {
    b: 0,
    c: {},
    d: {
        e: 1,
    },
    f: {
        g: 2,
        h: {
            i: 3,
        },
    },
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
