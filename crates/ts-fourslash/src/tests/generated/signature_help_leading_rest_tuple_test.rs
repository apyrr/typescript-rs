#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_leading_rest_tuple() {
    let mut t = TestingT;
    run_test_signature_help_leading_rest_tuple(&mut t);
}

fn run_test_signature_help_leading_rest_tuple(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpLeadingRestTuple") {
        return;
    }
    let content = r#"export function leading(...args: [...names: string[], allCaps: boolean]): void {
}

leading(/*1*/);
leading("ok", /*2*/);
leading("ok", "ok", /*3*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("leading(...names: string[], allCaps: boolean): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(2),
            overloads_count: 1,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("leading(...names: string[], allCaps: boolean): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(2),
            overloads_count: 1,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("leading(...names: string[], allCaps: boolean): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(2),
            overloads_count: 1,
        },
    );
    done();
}
