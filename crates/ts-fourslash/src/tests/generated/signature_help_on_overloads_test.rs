#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_on_overloads() {
    let mut t = TestingT;
    run_test_signature_help_on_overloads(&mut t);
}

fn run_test_signature_help_on_overloads(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpOnOverloads") {
        return;
    }
    let content = r"declare function fn(x: string);
declare function fn(x: string, y: number);
fn(/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("fn(x: string): any".to_string()),
            parameter_name: Some("x".to_string()),
            parameter_span: Some("x: string".to_string()),
            parameter_count: None,
            overloads_count: 2,
        },
    );
    f.insert(t, "'',");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("fn(x: string, y: number): any".to_string()),
            parameter_name: Some("y".to_string()),
            parameter_span: Some("y: number".to_string()),
            parameter_count: None,
            overloads_count: 2,
        },
    );
    done();
}
