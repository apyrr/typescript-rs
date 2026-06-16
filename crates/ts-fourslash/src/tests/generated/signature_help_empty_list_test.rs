#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_empty_list() {
    let mut t = TestingT;
    run_test_signature_help_empty_list(&mut t);
}

fn run_test_signature_help_empty_list(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpEmptyList") {
        return;
    }
    let content = r"function Foo(arg1: string, arg2: string) {
}

Foo(/*1*/);
function Bar<T>(arg1: string, arg2: string) { }
Bar</*2*/>();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("Foo(arg1: string, arg2: string): void".to_string()),
            parameter_name: Some("arg1".to_string()),
            parameter_span: Some("arg1: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("Bar<T>(arg1: string, arg2: string): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
