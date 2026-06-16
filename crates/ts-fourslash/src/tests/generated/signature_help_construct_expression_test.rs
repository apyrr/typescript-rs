#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_construct_expression() {
    let mut t = TestingT;
    run_test_signature_help_construct_expression(&mut t);
}

fn run_test_signature_help_construct_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpConstructExpression") {
        return;
    }
    let content = r#"class sampleCls { constructor(str: string, num: number) { } }
var x = new sampleCls(/*1*/"", /*2*/5);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("sampleCls(str: string, num: number): sampleCls".to_string()),
            parameter_name: Some("str".to_string()),
            parameter_span: Some("str: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("num".to_string()),
            parameter_span: Some("num: number".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
