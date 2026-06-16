#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_parameter_help() {
    let mut t = TestingT;
    run_test_generic_parameter_help(&mut t);
}

fn run_test_generic_parameter_help(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericParameterHelp") {
        return;
    }
    let content = r"interface IFoo { }

function testFunction<T extends IFoo, U, M extends IFoo>(a: T, b: U, c: M): M {
    return null;
}

// Function calls
testFunction</*1*/
testFunction<any, /*2*/
testFunction<any, any, any>(/*3*/
testFunction<any, any,/*4*/ any>(null, null, null);
testFunction<, ,/*5*/>(null, null, null);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some(
                "testFunction<T extends IFoo, U, M extends IFoo>(a: T, b: U, c: M): M".to_string(),
            ),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T extends IFoo".to_string()),
            parameter_count: Some(3),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("U".to_string()),
            parameter_span: Some("U".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("a".to_string()),
            parameter_span: Some("a: any".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "4");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("M".to_string()),
            parameter_span: Some("M extends IFoo".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "5");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("M".to_string()),
            parameter_span: Some("M extends IFoo".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
