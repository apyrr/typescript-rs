#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_assignment_types() {
    let mut t = TestingT;
    run_test_navigation_bar_assignment_types(&mut t);
}

fn run_test_navigation_bar_assignment_types(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarAssignmentTypes") {
        return;
    }
    let content = r"'use strict'
const a = {
    ...b,
    c,
    d: 0
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
