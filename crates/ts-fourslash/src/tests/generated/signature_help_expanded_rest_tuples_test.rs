#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_expanded_rest_tuples() {
    let mut t = TestingT;
    run_test_signature_help_expanded_rest_tuples(&mut t);
}

fn run_test_signature_help_expanded_rest_tuples(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpExpandedRestTuples") {
        return;
    }
    let content = r#"export function complex(item: string, another: string, ...rest: [] | [settings: object, errorHandler: (err: Error) => void] | [errorHandler: (err: Error) => void, ...mixins: object[]]) {
    
}

complex(/*1*/);
complex("ok", "ok", /*2*/);
complex("ok", "ok", e => void e, {}, /*3*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("complex(item: string, another: string): void".to_string()),
            parameter_name: Some("item".to_string()),
            parameter_span: Some("item: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 3,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(t, VerifySignatureHelpOptions {
    text: Some("complex(item: string, another: string, settings: object, errorHandler: (err: Error) => void): void".to_string()),
    parameter_name: Some("settings".to_string()),
    parameter_span: Some("settings: object".to_string()),
    parameter_count: Some(4),
    overloads_count: 3,
});
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(t, VerifySignatureHelpOptions {
    text: Some("complex(item: string, another: string, errorHandler: (err: Error) => void, ...mixins: object[]): void".to_string()),
    parameter_name: None,
    parameter_span: None,
    parameter_count: None,
    overloads_count: 3,
});
    done();
}
