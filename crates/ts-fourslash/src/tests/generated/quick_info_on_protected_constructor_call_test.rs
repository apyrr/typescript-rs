#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_protected_constructor_call() {
    let mut t = TestingT;
    run_test_quick_info_on_protected_constructor_call(&mut t);
}

fn run_test_quick_info_on_protected_constructor_call(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnProtectedConstructorCall") {
        return;
    }
    let content = r"class A {
    protected constructor() {}
}
var x = new A(/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &vec!["1".to_string()]);
    done();
}
