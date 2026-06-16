#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_array_binding_pattern01() {
    let mut t = TestingT;
    run_test_smart_indent_array_binding_pattern01(&mut t);
}

fn run_test_smart_indent_array_binding_pattern01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var /*1*/[/*2*/a,/*3*/b,/*4*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "2");
    f.insert(t, "\n");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "3");
    f.insert(t, "\n");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "4");
    f.insert(t, "\n");
    f.verify_indentation(t, 8);
    done();
}
