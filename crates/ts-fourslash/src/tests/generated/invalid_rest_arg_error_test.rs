#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_invalid_rest_arg_error() {
    let mut t = TestingT;
    run_test_invalid_rest_arg_error(&mut t);
}

fn run_test_invalid_rest_arg_error(t: &mut TestingT) {
    if should_skip_if_failing("TestInvalidRestArgError") {
        return;
    }
    let content = r"function b(.../*1*/)/*2*/ {}  ";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"));
    done();
}
