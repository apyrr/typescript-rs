#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_in_parenthetical() {
    let mut t = TestingT;
    run_test_signature_help_in_parenthetical(&mut t);
}

fn run_test_signature_help_in_parenthetical(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class base { constructor (public n: number, public y: string) { } }
(new base(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("n".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.insert(t, "0, ");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("y".to_string()),
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
