#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_with_invalid_argument_list1() {
    let mut t = TestingT;
    run_test_signature_help_with_invalid_argument_list1(&mut t);
}

fn run_test_signature_help_with_invalid_argument_list1(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpWithInvalidArgumentList1") {
        return;
    }
    let content = r"function foo(a) { }
foo(hello my name /**/is";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(a: any): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
