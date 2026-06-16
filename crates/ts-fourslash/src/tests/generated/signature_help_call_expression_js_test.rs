#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_call_expression_js() {
    let mut t = TestingT;
    run_test_signature_help_call_expression_js(&mut t);
}

fn run_test_signature_help_call_expression_js(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
// @checkJs: true
// @allowJs: true
// @Filename: main.js
function allOptional() { arguments; }
allOptional(/*1*/);
allOptional(1, 2, 3);
function someOptional(x, y) { arguments; }
someOptional(/*2*/);
someOptional(1, 2, 3);
someOptional(); // no error here; x and y are optional in JS";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("allOptional(...args: any[]): void".to_string()),
            parameter_name: Some("args".to_string()),
            parameter_span: Some("...args: any[]".to_string()),
            parameter_count: Some(1),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("someOptional(x: any, y: any, ...args: any[]): void".to_string()),
            parameter_name: Some("x".to_string()),
            parameter_span: Some("x: any".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    done();
}
