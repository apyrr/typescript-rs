#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_hierarchy_class_property_arrow_function() {
    let mut t = TestingT;
    run_test_call_hierarchy_class_property_arrow_function(&mut t);
}

fn run_test_call_hierarchy_class_property_arrow_function(t: &mut TestingT) {
    if should_skip_if_failing("TestCallHierarchyClassPropertyArrowFunction") {
        return;
    }
    let content = r"class C {
    caller = () => {
        this.callee();
    }

    /**/callee = () => {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_call_hierarchy(t);
    done();
}
