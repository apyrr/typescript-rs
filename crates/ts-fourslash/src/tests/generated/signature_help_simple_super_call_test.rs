#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_simple_super_call() {
    let mut t = TestingT;
    run_test_signature_help_simple_super_call(&mut t);
}

fn run_test_signature_help_simple_super_call(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class SuperCallBase {
    constructor(b: boolean) {
    }
}
class SuperCall extends SuperCallBase {
    constructor() {
        super(/*superCall*/);
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "superCall");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("SuperCallBase(b: boolean): SuperCallBase".to_string()),
            parameter_name: Some("b".to_string()),
            parameter_span: Some("b: boolean".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
