#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_this() {
    let mut t = TestingT;
    run_test_signature_help_this(&mut t);
}

fn run_test_signature_help_this(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpThis") {
        return;
    }
    let content = r"class Foo<T> {
    public implicitAny(n: number) {
    }
    public explicitThis(this: this, n: number) {
        console.log(this);
    }
    public explicitClass(this: Foo<T>, n: number) {
        console.log(this);
    }
}

function implicitAny(x: number): void {
    return this;
}
function explicitVoid(this: void, x: number): void {
    return this;
}
function explicitLiteral(this: { n: number }, x: number): void {
    console.log(this);
}
let foo = new Foo<number>();
foo.implicitAny(/*1*/);
foo.explicitThis(/*2*/);
foo.explicitClass(/*3*/);
implicitAny(/*4*/12);
explicitVoid(/*5*/13);
let o = { n: 14, m: explicitLiteral };
o.m(/*6*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("n".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("n".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("n".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "4");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("x".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "5");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("x".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "6");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("x".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
