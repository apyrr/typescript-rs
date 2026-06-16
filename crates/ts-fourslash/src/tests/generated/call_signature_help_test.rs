#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_signature_help() {
    let mut t = TestingT;
    run_test_call_signature_help(&mut t);
}

fn run_test_call_signature_help(t: &mut TestingT) {
    if should_skip_if_failing("TestCallSignatureHelp") {
        return;
    }
    let content = r"interface C {
   (): number;
}
var c: C;
c(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("c(): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
