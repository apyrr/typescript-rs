#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_trailing_rest_tuple() {
    let mut t = TestingT;
    run_test_signature_help_trailing_rest_tuple(&mut t);
}

fn run_test_signature_help_trailing_rest_tuple(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"export function leading(allCaps: boolean, ...names: string[]): void {
}

leading(/*1*/);
leading(false, /*2*/);
leading(false, "ok", /*3*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("leading(allCaps: boolean, ...names: string[]): void".to_string()),
            parameter_name: Some("allCaps".to_string()),
            parameter_span: Some("allCaps: boolean".to_string()),
            parameter_count: Some(2),
            overloads_count: 1,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("leading(allCaps: boolean, ...names: string[]): void".to_string()),
            parameter_name: Some("names".to_string()),
            parameter_span: Some("...names: string[]".to_string()),
            parameter_count: Some(2),
            overloads_count: 1,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("leading(allCaps: boolean, ...names: string[]): void".to_string()),
            parameter_name: Some("names".to_string()),
            parameter_span: Some("...names: string[]".to_string()),
            parameter_count: Some(2),
            overloads_count: 1,
        },
    );
    done();
}
