#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_parameter_info_on_parameter_type() {
    let mut t = TestingT;
    run_test_parameter_info_on_parameter_type(&mut t);
}

fn run_test_parameter_info_on_parameter_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function foo(a: string) { };
var b = "test";
foo("test"/*1*/);
foo(b/*2*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.go_to_marker(t, &marker);
        f.verify_signature_help_options(
            t,
            VerifySignatureHelpOptions {
                text: None,
                parameter_name: Some("a".to_string()),
                parameter_span: None,
                parameter_count: None,
                overloads_count: 0,
            },
        );
    }
    done();
}
