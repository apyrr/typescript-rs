#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_optional_call() {
    let mut t = TestingT;
    run_test_signature_help_optional_call(&mut t);
}

fn run_test_signature_help_optional_call(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function fnTest(str: string, num: number) { }
fnTest?.(/*1*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("fnTest(str: string, num: number): void".to_string()),
            parameter_name: Some("str".to_string()),
            parameter_span: Some("str: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    done();
}
