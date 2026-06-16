#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_function_return_type2() {
    let mut t = TestingT;
    run_test_generic_function_return_type2(&mut t);
}

fn run_test_generic_function_return_type2(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericFunctionReturnType2") {
        return;
    }
    let content = r"class C<T> {
    constructor(x: T) { }
    foo(x: T) {
        return (a: T) => x;
    }
}
var x = new C(1);
var /*2*/r = x.foo(/*1*/3);
var /*4*/r2 = r(/*3*/4);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(x: number): (a: number) => number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_quick_info_at(t, "2", "var r: (a: number) => number", "");
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("r(a: number): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_quick_info_at(t, "4", "var r2: number", "");
    done();
}
