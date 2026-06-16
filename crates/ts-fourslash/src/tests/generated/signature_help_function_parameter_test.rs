#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_function_parameter() {
    let mut t = TestingT;
    run_test_signature_help_function_parameter(&mut t);
}

fn run_test_signature_help_function_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpFunctionParameter") {
        return;
    }
    let content = r#"function parameterFunction(callback: (a: number, b: string) => void) {
    callback(/*parameterFunction1*/5, /*parameterFunction2*/"");
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "parameterFunction1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("callback(a: number, b: string): void".to_string()),
            parameter_name: Some("a".to_string()),
            parameter_span: Some("a: number".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "parameterFunction2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("callback(a: number, b: string): void".to_string()),
            parameter_name: Some("b".to_string()),
            parameter_span: Some("b: string".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
