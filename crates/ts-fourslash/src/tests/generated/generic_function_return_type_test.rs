#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_function_return_type() {
    let mut t = TestingT;
    run_test_generic_function_return_type(&mut t);
}

fn run_test_generic_function_return_type(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericFunctionReturnType") {
        return;
    }
    let content = r#"function foo<T, U>(x: T, y: U): (a: U) => T {
    var z = y;
    return (z) => x;
}
var /*2*/r = foo(/*1*/1, "");
var /*4*/r2 = r(/*3*/"");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(x: number, y: string): (a: string) => number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_quick_info_at(t, "2", "var r: (a: string) => number", "");
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("r(a: string): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_quick_info_at(t, "4", "var r2: number", "");
    done();
}
