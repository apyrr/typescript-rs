#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_expanded_tuples_argument_index() {
    let mut t = TestingT;
    run_test_signature_help_expanded_tuples_argument_index(&mut t);
}

fn run_test_signature_help_expanded_tuples_argument_index(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpExpandedTuplesArgumentIndex") {
        return;
    }
    let content = r#"function foo(...args: [string, string] | [number, string, string]
) {

}

foo(123/*1*/,)
foo(""/*2*/, ""/*3*/)
foo(123/*4*/, ""/*5*/, )
foo(123/*6*/, ""/*7*/, ""/*8*/)"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: number, args_1: string, args_2: string): void".to_string()),
            parameter_name: Some("args_0".to_string()),
            parameter_span: Some("args_0: number".to_string()),
            parameter_count: Some(3),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: string, args_1: string): void".to_string()),
            parameter_name: Some("args_0".to_string()),
            parameter_span: Some("args_0: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: string, args_1: string): void".to_string()),
            parameter_name: Some("args_1".to_string()),
            parameter_span: Some("args_1: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "4");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: number, args_1: string, args_2: string): void".to_string()),
            parameter_name: Some("args_0".to_string()),
            parameter_span: Some("args_0: number".to_string()),
            parameter_count: Some(3),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "5");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: number, args_1: string, args_2: string): void".to_string()),
            parameter_name: Some("args_1".to_string()),
            parameter_span: Some("args_1: string".to_string()),
            parameter_count: Some(3),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "6");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: number, args_1: string, args_2: string): void".to_string()),
            parameter_name: Some("args_0".to_string()),
            parameter_span: Some("args_0: number".to_string()),
            parameter_count: Some(3),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "7");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: number, args_1: string, args_2: string): void".to_string()),
            parameter_name: Some("args_1".to_string()),
            parameter_span: Some("args_1: string".to_string()),
            parameter_count: Some(3),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "8");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(args_0: number, args_1: string, args_2: string): void".to_string()),
            parameter_name: Some("args_2".to_string()),
            parameter_span: Some("args_2: string".to_string()),
            parameter_count: Some(3),
            overloads_count: 2,
        },
    );
    done();
}
