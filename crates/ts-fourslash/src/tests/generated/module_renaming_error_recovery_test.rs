#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_module_renaming_error_recovery() {
    let mut t = TestingT;
    run_test_module_renaming_error_recovery(&mut t);
}

fn run_test_module_renaming_error_recovery(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace Alpha/*1*//*2*/ { class Foo { public bar() { } } }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.backspace(t, 5);
    f.insert(t, "Pizza");
    f.verify_current_line_content(t, "namespace Pizza { class Foo { public bar() { } } }");
    f.verify_no_error_exists_after_marker_name("2");
    done();
}
