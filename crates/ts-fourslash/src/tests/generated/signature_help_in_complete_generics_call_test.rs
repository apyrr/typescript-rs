#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_in_complete_generics_call() {
    let mut t = TestingT;
    run_test_signature_help_in_complete_generics_call(&mut t);
}

fn run_test_signature_help_in_complete_generics_call(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo<T>(x: number, callback: (x: T) => number) {
}
foo(/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(x: number, callback: (x: unknown) => number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
