#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_parameter_help_type_references() {
    let mut t = TestingT;
    run_test_generic_parameter_help_type_references(&mut t);
}

fn run_test_generic_parameter_help_type_references(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericParameterHelpTypeReferences") {
        return;
    }
    let content = r"interface IFoo { }

class testClass<T extends IFoo, U, M extends IFoo> {
    constructor(a:T, b:U, c:M){ }
}

// Generic types
testClass</*type1*/
var x : testClass</*type2*/
class Bar<T> extends testClass</*type3*/
var x : testClass<,, /*type4*/any>;

interface I<T> {}
let i: I</*interface*/>;

type Ty<T> = T;
let t: Ty</*typeAlias*/>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "type1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("testClass<T extends IFoo, U, M extends IFoo>".to_string()),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T extends IFoo".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "type2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("testClass<T extends IFoo, U, M extends IFoo>".to_string()),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T extends IFoo".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "type3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("testClass<T extends IFoo, U, M extends IFoo>".to_string()),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T extends IFoo".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "type4");
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
    f.go_to_marker(t, "interface");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("I<T>".to_string()),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "typeAlias");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("Ty<T>".to_string()),
            parameter_name: Some("T".to_string()),
            parameter_span: Some("T".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
