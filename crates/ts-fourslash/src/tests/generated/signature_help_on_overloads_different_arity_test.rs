#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_on_overloads_different_arity() {
    let mut t = TestingT;
    run_test_signature_help_on_overloads_different_arity(&mut t);
}

fn run_test_signature_help_on_overloads_different_arity(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare function f(s: string);
declare function f(n: number);
declare function f(s: string, b: boolean);
declare function f(n: number, b: boolean);

f(1/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(n: number): any".to_string()),
            parameter_name: Some("n".to_string()),
            parameter_span: Some("n: number".to_string()),
            parameter_count: None,
            overloads_count: 4,
        },
    );
    f.insert(t, ", ");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f(n: number, b: boolean): any".to_string()),
            parameter_name: Some("b".to_string()),
            parameter_span: Some("b: boolean".to_string()),
            parameter_count: None,
            overloads_count: 4,
        },
    );
    done();
}
