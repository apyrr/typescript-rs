#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_with_triggers02() {
    let mut t = TestingT;
    run_test_signature_help_with_triggers02(&mut t);
}

fn run_test_signature_help_with_triggers02(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpWithTriggers02") {
        return;
    }
    let content = r"declare function foo<T>(x: T, y: T): T;
declare function bar<U>(x: U, y: U): U;

foo(bar/*1*/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "(");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("bar(x: unknown, y: unknown): unknown".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.backspace(t, 1);
    f.insert(t, "<");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("bar<U>(x: U, y: U): U".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.backspace(t, 1);
    f.insert(t, ",");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some(
                "foo(x: <U>(x: U, y: U) => U, y: <U>(x: U, y: U) => U): <U>(x: U, y: U) => U"
                    .to_string(),
            ),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.backspace(t, 1);
    done();
}
