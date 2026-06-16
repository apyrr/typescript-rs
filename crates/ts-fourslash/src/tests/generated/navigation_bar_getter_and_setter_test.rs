#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_getter_and_setter() {
    let mut t = TestingT;
    run_test_navigation_bar_getter_and_setter(&mut t);
}

fn run_test_navigation_bar_getter_and_setter(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarGetterAndSetter") {
        return;
    }
    let content = r"class X {
    get x() {}
    set x(value) {
        // Inner declaration should make the setter top-level.
        function f() {}
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
