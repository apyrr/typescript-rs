#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_duplicate_setter() {
    let mut t = TestingT;
    run_test_add_duplicate_setter(&mut t);
}

fn run_test_add_duplicate_setter(t: &mut TestingT) {
    if should_skip_if_failing("TestAddDuplicateSetter") {
        return;
    }
    let content = r"class C {
    set foo(value) { }
    /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "set foo(value) { }");
    done();
}
