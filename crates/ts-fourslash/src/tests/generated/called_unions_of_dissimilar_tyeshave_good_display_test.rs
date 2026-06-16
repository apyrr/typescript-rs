#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_called_unions_of_dissimilar_tyeshave_good_display() {
    let mut t = TestingT;
    run_test_called_unions_of_dissimilar_tyeshave_good_display(&mut t);
}

fn run_test_called_unions_of_dissimilar_tyeshave_good_display(t: &mut TestingT) {
    if should_skip_if_failing("TestCalledUnionsOfDissimilarTyeshaveGoodDisplay") {
        return;
    }
    let content = r"declare const callableThing1:
    | ((o1: {x: number}) => void)
    | ((o1: {y: number}) => void)
    ;

callableThing1(/*1*/);

declare const callableThing2:
    | ((o1: {x: number}) => void)
    | ((o2: {y: number}) => void)
    ;

callableThing2(/*2*/);

declare const callableThing3:
    | ((o1: {x: number}) => void)
    | ((o2: {y: number}) => void)
    | ((o3: {z: number}) => void)
    | ((o4: {u: number}) => void)
    | ((o5: {v: number}) => void)
    ;

callableThing3(/*3*/);

declare const callableThing4:
    | ((o1: {x: number}) => void)
    | ((o2: {y: number}) => void)
    | ((o3: {z: number}) => void)
    | ((o4: {u: number}) => void)
    | ((o5: {v: number}) => void)
    | ((o6: {w: number}) => void)
    ;

callableThing4(/*4*/);

declare const callableThing5: 
    | (<U>(a1: U) => void)
    | (() => void) 
    ;

callableThing5(/*5*/1)
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("callableThing1(o1: { x: number; } & { y: number; }): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("callableThing2(arg0: { x: number; } & { y: number; }): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(t, VerifySignatureHelpOptions {
    text: Some("callableThing3(arg0: { x: number; } & { y: number; } & { z: number; } & { u: number; } & { v: number; }): void".to_string()),
    parameter_name: None,
    parameter_span: None,
    parameter_count: None,
    overloads_count: 0,
});
    f.go_to_marker(t, "4");
    f.verify_signature_help_options(t, VerifySignatureHelpOptions {
    text: Some("callableThing4(arg0: { x: number; } & { y: number; } & { z: number; } & { u: number; } & { v: number; } & { w: number; }): void".to_string()),
    parameter_name: None,
    parameter_span: None,
    parameter_count: None,
    overloads_count: 0,
});
    f.go_to_marker(t, "5");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("callableThing5(a1: number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
