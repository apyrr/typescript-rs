#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_for_super_calls1() {
    let mut t = TestingT;
    run_test_signature_help_for_super_calls1(&mut t);
}

fn run_test_signature_help_for_super_calls1(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpForSuperCalls1") {
        return;
    }
    let content = r"class A { }
class B extends A { }
class C extends B {
    constructor() {
        super(/*1*/ // sig help here?
    }
}
class A2 { }
class B2 extends A2 {
    constructor(x:number) {}
 }
class C2 extends B2 {
    constructor() {
        super(/*2*/ // sig help here?
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B(): B".to_string()),
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
            text: Some("B2(x: number): B2".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
