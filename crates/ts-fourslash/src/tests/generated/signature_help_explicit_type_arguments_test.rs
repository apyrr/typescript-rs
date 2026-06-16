#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_explicit_type_arguments() {
    let mut t = TestingT;
    run_test_signature_help_explicit_type_arguments(&mut t);
}

fn run_test_signature_help_explicit_type_arguments(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpExplicitTypeArguments") {
        return;
    }
    let content = r"declare function f<T = boolean, U = string>(x: T, y: U): T;
f<number, string>(/*1*/);
f(/*2*/);
f<number>(/*3*/);
f<number, string, boolean>(/*4*/);
interface A { a: number }
interface B extends A { b: string }
declare function g<T, U, V extends A = B>(x: T, y: U, z: V): T;
declare function h<T, U, V extends A>(x: T, y: U, z: V): T;
declare function j<T, U, V = B>(x: T, y: U, z: V): T;
g(/*5*/);
h(/*6*/);
j(/*7*/);
g<number>(/*8*/);
h<number>(/*9*/);
j<number>(/*10*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(x: number, y: string): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(x: boolean, y: string): boolean".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(x: number, y: string): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "4");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(x: number, y: string): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "5");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("g(x: unknown, y: unknown, z: B): unknown".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "6");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("h(x: unknown, y: unknown, z: A): unknown".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "7");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("j(x: unknown, y: unknown, z: B): unknown".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "8");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("g(x: number, y: unknown, z: B): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "9");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("h(x: number, y: unknown, z: A): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "10");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("j(x: number, y: unknown, z: B): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
