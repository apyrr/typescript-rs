#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_implicit_constructor() {
    let mut t = TestingT;
    run_test_signature_help_implicit_constructor(&mut t);
}

fn run_test_signature_help_implicit_constructor(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class ImplicitConstructor {
}
var implicitConstructor = new ImplicitConstructor(/**/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("ImplicitConstructor(): ImplicitConstructor".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 0,
        },
    );
    done();
}
