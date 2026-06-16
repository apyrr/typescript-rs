#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_anonymous_function() {
    let mut t = TestingT;
    run_test_signature_help_anonymous_function(&mut t);
}

fn run_test_signature_help_anonymous_function(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var anonymousFunctionTest = function(n: number, s: string): (a: number, b: string) => string {
    return null;
}
anonymousFunctionTest(5, "")(/*anonymousFunction1*/1, /*anonymousFunction2*/"");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "anonymousFunction1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("(a: number, b: string): string".to_string()),
            parameter_name: Some("a".to_string()),
            parameter_span: Some("a: number".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "anonymousFunction2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("b".to_string()),
            parameter_span: Some("b: string".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
