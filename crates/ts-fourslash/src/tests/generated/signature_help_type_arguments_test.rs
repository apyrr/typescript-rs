#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_type_arguments() {
    let mut t = TestingT;
    run_test_signature_help_type_arguments(&mut t);
}

fn run_test_signature_help_type_arguments(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare function f(a: number, b: string, c: boolean): void; // ignored, not generic
declare function f<T extends number>(): void;
declare function f<T, U>(): void;
declare function f<T, U, V extends string>(): void;
f</*f0*/;
f<number, /*f1*/;
f<number, string, /*f2*/;

declare const C: {
    new<T extends number>(): void;
    new<T, U>(): void;
    new<T, U, V extends string>(): void;
};
new C</*C0*/;
new C<number, /*C1*/;
new C<number, string, /*C2*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "f0");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f<T extends number>(): void".to_string()),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T extends number".to_string()),
            parameter_count: None,
            overloads_count: 3,
        },
    );
    f.go_to_marker(t, "f1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f<T, U>(): void".to_string()),
            parameter_name: Some("U".to_string()),
            parameter_span: Some("U".to_string()),
            parameter_count: None,
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "f2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f<T, U, V extends string>(): void".to_string()),
            parameter_name: Some("V".to_string()),
            parameter_span: Some("V extends string".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "C0");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("C<T extends number>(): void".to_string()),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T extends number".to_string()),
            parameter_count: None,
            overloads_count: 3,
        },
    );
    f.go_to_marker(t, "C1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("C<T, U>(): void".to_string()),
            parameter_name: Some("U".to_string()),
            parameter_span: Some("U".to_string()),
            parameter_count: None,
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "C2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("C<T, U, V extends string>(): void".to_string()),
            parameter_name: Some("V".to_string()),
            parameter_span: Some("V extends string".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
