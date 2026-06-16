#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_no_signature_help_on_new_keyword() {
    let mut t = TestingT;
    run_test_no_signature_help_on_new_keyword(&mut t);
}

fn run_test_no_signature_help_on_new_keyword(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Foo { }
new/*1*/ Foo
new /*2*/Foo(/*3*/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &vec!["1".to_string(), "2".to_string()]);
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("Foo(): Foo".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
