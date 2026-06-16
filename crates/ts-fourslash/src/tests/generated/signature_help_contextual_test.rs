#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_contextual() {
    let mut t = TestingT;
    run_test_signature_help_contextual(&mut t);
}

fn run_test_signature_help_contextual(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    m(n: number, s: string): void;
    m2: () => void;
}
declare function takesObj(i: I): void;
takesObj({ m: (/*takesObj0*/) });
takesObj({ m(/*takesObj1*/) });
takesObj({ m: function(/*takesObj2*/) });
takesObj({ m2: (/*takesObj3*/) });

declare function takesCb(cb: (n: number, s: string, b: boolean) => void): void;
takesCb((/*contextualParameter1*/));
takesCb((/*contextualParameter1b*/) => {});
takesCb((n, /*contextualParameter2*/));
takesCb((n, s, /*contextualParameter3*/));
takesCb((n,/*contextualParameter3_2*/ s, b));
takesCb((n, s, b, /*contextualParameter4*/));

type Cb = () => void;
const cb: Cb = (/*contextualTypeAlias*/)

const cb2: () => void = (/*contextualFunctionType*/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "takesObj0");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("m(n: number, s: string): void".to_string()),
            parameter_name: Some("n".to_string()),
            parameter_span: Some("n: number".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "takesObj1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("m(n: number, s: string): void".to_string()),
            parameter_name: Some("n".to_string()),
            parameter_span: Some("n: number".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "takesObj2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("m(n: number, s: string): void".to_string()),
            parameter_name: Some("n".to_string()),
            parameter_span: Some("n: number".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "takesObj3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("m2(): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualParameter1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("cb(n: number, s: string, b: boolean): void".to_string()),
            parameter_name: Some("n".to_string()),
            parameter_span: Some("n: number".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualParameter1b");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("cb(n: number, s: string, b: boolean): void".to_string()),
            parameter_name: Some("n".to_string()),
            parameter_span: Some("n: number".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualParameter2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("cb(n: number, s: string, b: boolean): void".to_string()),
            parameter_name: Some("s".to_string()),
            parameter_span: Some("s: string".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualParameter3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("cb(n: number, s: string, b: boolean): void".to_string()),
            parameter_name: Some("b".to_string()),
            parameter_span: Some("b: boolean".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualParameter3_2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("cb(n: number, s: string, b: boolean): void".to_string()),
            parameter_name: Some("s".to_string()),
            parameter_span: Some("s: string".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualParameter4");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("cb(n: number, s: string, b: boolean): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualTypeAlias");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("Cb(): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "contextualFunctionType");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("cb2(): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 0,
        },
    );
    done();
}
