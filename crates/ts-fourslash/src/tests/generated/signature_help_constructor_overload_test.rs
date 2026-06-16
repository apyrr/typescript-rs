#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_constructor_overload() {
    let mut t = TestingT;
    run_test_signature_help_constructor_overload(&mut t);
}

fn run_test_signature_help_constructor_overload(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class clsOverload { constructor(); constructor(test: string); constructor(test?: string) { } }
var x = new clsOverload(/*1*/);
var y = new clsOverload(/*2*/'');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("clsOverload(): clsOverload".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 2,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("clsOverload(test: string): clsOverload".to_string()),
            parameter_name: Some("test".to_string()),
            parameter_span: Some("test: string".to_string()),
            parameter_count: Some(1),
            overloads_count: 2,
        },
    );
    done();
}
