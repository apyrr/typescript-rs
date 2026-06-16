#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_simple_constructor_call() {
    let mut t = TestingT;
    run_test_signature_help_simple_constructor_call(&mut t);
}

fn run_test_signature_help_simple_constructor_call(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class ConstructorCall {
    constructor(str: string, num: number) {
    }
}
var x = new ConstructorCall(/*constructorCall1*/1,/*constructorCall2*/2);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "constructorCall1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("ConstructorCall(str: string, num: number): ConstructorCall".to_string()),
            parameter_name: Some("str".to_string()),
            parameter_span: Some("str: string".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "constructorCall2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("ConstructorCall(str: string, num: number): ConstructorCall".to_string()),
            parameter_name: Some("num".to_string()),
            parameter_span: Some("num: number".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
