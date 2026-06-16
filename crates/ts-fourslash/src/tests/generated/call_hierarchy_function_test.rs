#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_hierarchy_function() {
    let mut t = TestingT;
    run_test_call_hierarchy_function(&mut t);
}

fn run_test_call_hierarchy_function(t: &mut TestingT) {
    if should_skip_if_failing("TestCallHierarchyFunction") {
        return;
    }
    let content = r"function foo() {
    bar();
}

function /**/bar() {
    baz();
    quxx();
    baz();
}

function baz() {
}

function quxx() {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_call_hierarchy(t);
    done();
}
