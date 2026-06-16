#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_in_callback() {
    let mut t = TestingT;
    run_test_signature_help_in_callback(&mut t);
}

fn run_test_signature_help_in_callback(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpInCallback") {
        return;
    }
    let content = r"declare function forEach(f: () => void);
forEach(/*1*/() => {
    /*2*/
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("forEach(f: () => void): any".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_no_signature_help_for_markers(t, &vec!["2".to_string()]);
    done();
}
