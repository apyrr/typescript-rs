#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_on_overloads_different_arity3() {
    let mut t = TestingT;
    run_test_signature_help_on_overloads_different_arity3(&mut t);
}

fn run_test_signature_help_on_overloads_different_arity3(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpOnOverloadsDifferentArity3") {
        return;
    }
    let content = r"declare function f();
declare function f(s: string);
declare function f(s: string, b: boolean);
declare function f(n: number, b: boolean);

f(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(): any".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 4,
        },
    );
    f.insert(t, "x, ");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(s: string, b: boolean): any".to_string()),
            parameter_name: Some("b".to_string()),
            parameter_span: Some("b: boolean".to_string()),
            parameter_count: Some(2),
            overloads_count: 4,
        },
    );
    f.insert(t, "x, ");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(s: string, b: boolean): any".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(2),
            overloads_count: 4,
        },
    );
    done();
}
