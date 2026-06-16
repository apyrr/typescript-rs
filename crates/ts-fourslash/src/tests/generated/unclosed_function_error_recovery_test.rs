#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unclosed_function_error_recovery() {
    let mut t = TestingT;
    run_test_unclosed_function_error_recovery(&mut t);
}

fn run_test_unclosed_function_error_recovery(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function alpha() {

function beta() { /*1*/alpha()/*2*/; }
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"));
    done();
}
