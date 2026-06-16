#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_incomplete_calls() {
    let mut t = TestingT;
    run_test_signature_help_incomplete_calls(&mut t);
}

fn run_test_signature_help_incomplete_calls(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"namespace IncompleteCalls {
    class Foo {
        public f1() { }
        public f2(n: number): number { return 0; }
        public f3(n: number, s: string) : string { return ""; }
    }
    var x = new Foo();
    x.f1();
    x.f2(5);
    x.f3(5, "");
    x.f1(/*incompleteCalls1*/
    x.f2(5,/*incompleteCalls2*/
    x.f3(5,/*incompleteCalls3*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "incompleteCalls1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f1(): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "incompleteCalls2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f2(n: number): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(1),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "incompleteCalls3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f3(n: number, s: string): string".to_string()),
            parameter_name: Some("s".to_string()),
            parameter_span: Some("s: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    done();
}
