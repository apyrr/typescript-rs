#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_simple_function_call() {
    let mut t = TestingT;
    run_test_signature_help_simple_function_call(&mut t);
}

fn run_test_signature_help_simple_function_call(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpSimpleFunctionCall") {
        return;
    }
    let content = r#"// Simple function test
function functionCall(str: string, num: number) {
}
functionCall(/*functionCall1*/);
functionCall("", /*functionCall2*/1);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "functionCall1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("functionCall(str: string, num: number): void".to_string()),
            parameter_name: Some("str".to_string()),
            parameter_span: Some("str: string".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "functionCall2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("functionCall(str: string, num: number): void".to_string()),
            parameter_name: Some("num".to_string()),
            parameter_span: Some("num: number".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
