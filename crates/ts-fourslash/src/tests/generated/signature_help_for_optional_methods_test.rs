#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_for_optional_methods() {
    let mut t = TestingT;
    run_test_signature_help_for_optional_methods(&mut t);
}

fn run_test_signature_help_for_optional_methods(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: true
interface Obj {
    optionalMethod?: (current: any) => any;
};

const o: Obj = {
  optionalMethod(/*1*/) {
    return {};
  }
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("optionalMethod(current: any): any".to_string()),
            parameter_name: Some("current".to_string()),
            parameter_span: Some("current: any".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
