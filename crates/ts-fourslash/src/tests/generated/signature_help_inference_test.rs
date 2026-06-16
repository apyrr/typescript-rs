#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_inference() {
    let mut t = TestingT;
    run_test_signature_help_inference(&mut t);
}

fn run_test_signature_help_inference(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"declare function f<T extends string>(a: T, b: T, c: T): void;
f("x", /**/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(a: \"x\", b: \"x\", c: \"x\"): void".to_string()),
            parameter_name: Some("b".to_string()),
            parameter_span: Some("b: \"x\"".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    done();
}
