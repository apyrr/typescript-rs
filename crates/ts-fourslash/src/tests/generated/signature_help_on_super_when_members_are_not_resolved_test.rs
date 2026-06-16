#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_on_super_when_members_are_not_resolved() {
    let mut t = TestingT;
    run_test_signature_help_on_super_when_members_are_not_resolved(&mut t);
}

fn run_test_signature_help_on_super_when_members_are_not_resolved(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpOnSuperWhenMembersAreNotResolved") {
        return;
    }
    let content = r"class A { }
class B extends A { constructor(public x: string) { } }
class C extends B {
    constructor() {
        /*1*/
     }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "super(");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B(x: string): B".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
