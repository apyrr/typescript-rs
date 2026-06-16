#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_parameter_help_constructor_calls() {
    let mut t = TestingT;
    run_test_generic_parameter_help_constructor_calls(&mut t);
}

fn run_test_generic_parameter_help_constructor_calls(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericParameterHelpConstructorCalls") {
        return;
    }
    let content = r"interface IFoo { }

class testClass<T extends IFoo, U, M extends IFoo> {
    constructor(a:T, b:U, c:M){ }
}

// Constructor calls
new testClass</*constructor1*/
new testClass<IFoo, /*constructor2*/
new testClass</*constructor3*/>(null, null, null)
new testClass<,,/*constructor4*/>(null, null, null)
new testClass<IFoo,/*constructor5*/IFoo,IFoo>(null, null, null)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "constructor1");
    f.verify_signature_help_options(t, VerifySignatureHelpOptions {
    text: Some("testClass<T extends IFoo, U, M extends IFoo>(a: T, b: U, c: M): testClass<T, U, M>".to_string()),
    parameter_name: Some("T".to_string()),
    parameter_span: Some("T extends IFoo".to_string()),
    parameter_count: None,
    overloads_count: 0,
});
    f.go_to_marker(t, "constructor2");
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
    f.go_to_marker(t, "constructor3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T extends IFoo".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "constructor4");
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
    f.go_to_marker(t, "constructor5");
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
    done();
}
