#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_hierarchy_accessor() {
    let mut t = TestingT;
    run_test_call_hierarchy_accessor(&mut t);
}

fn run_test_call_hierarchy_accessor(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo() {
    new C().bar;
}

class C {
    get /**/bar() {
        return baz();
    }
}

function baz() {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_call_hierarchy(t);
    done();
}
