#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_edit_lambda_arg_to_type_parameter1() {
    let mut t = TestingT;
    run_test_edit_lambda_arg_to_type_parameter1(&mut t);
}

fn run_test_edit_lambda_arg_to_type_parameter1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C<T> {
    foo(x: T) {
        return (a: number/*1*/) => x;
    }
}
/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.backspace(t, 6);
    f.insert(t, "T");
    f.verify_no_errors();
    f.go_to_marker(t, "2");
    f.insert_line(t, "");
    f.verify_no_errors();
    done();
}
