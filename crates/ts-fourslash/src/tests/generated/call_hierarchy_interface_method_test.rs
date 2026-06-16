#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_hierarchy_interface_method() {
    let mut t = TestingT;
    run_test_call_hierarchy_interface_method(&mut t);
}

fn run_test_call_hierarchy_interface_method(t: &mut TestingT) {
    if should_skip_if_failing("TestCallHierarchyInterfaceMethod") {
        return;
    }
    let content = r"interface I {
    /**/foo(): void;
}

const obj: I = { foo() {} };

obj.foo();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_call_hierarchy(t);
    done();
}
