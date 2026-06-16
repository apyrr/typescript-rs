#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rest_arg_signature_help() {
    let mut t = TestingT;
    run_test_rest_arg_signature_help(&mut t);
}

fn run_test_rest_arg_signature_help(t: &mut TestingT) {
    if should_skip_if_failing("TestRestArgSignatureHelp") {
        return;
    }
    let content = r"function f(...x: any[]) { }
f(/**/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
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
