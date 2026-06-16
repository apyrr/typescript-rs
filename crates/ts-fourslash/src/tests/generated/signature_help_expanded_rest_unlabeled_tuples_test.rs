#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_expanded_rest_unlabeled_tuples() {
    let mut t = TestingT;
    run_test_signature_help_expanded_rest_unlabeled_tuples(&mut t);
}

fn run_test_signature_help_expanded_rest_unlabeled_tuples(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpExpandedRestUnlabeledTuples") {
        return;
    }
    let content = r#"export function complex(item: string, another: string, ...rest: [] | [object, (err: Error) => void] | [(err: Error) => void, ...object[]]) {
    
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
    text: Some("complex(item: string, another: string, rest_0: object, rest_1: (err: Error) => void): void".to_string()),
    parameter_name: Some("rest_0".to_string()),
    parameter_span: Some("rest_0: object".to_string()),
    parameter_count: Some(4),
    overloads_count: 3,
});
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(t, VerifySignatureHelpOptions {
    text: Some("complex(item: string, another: string, rest_0: (err: Error) => void, ...rest: object[]): void".to_string()),
    parameter_name: None,
    parameter_span: None,
    parameter_count: None,
    overloads_count: 3,
});
    done();
}
