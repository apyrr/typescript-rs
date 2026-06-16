#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_static_generic_overloads1() {
    let mut t = TestingT;
    run_test_static_generic_overloads1(&mut t);
}

fn run_test_static_generic_overloads1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class A<T> {
    static B<S>(v: A<S>): A<S>;
    static B<S>(v: S): A<S>;
    static B<S>(v: any): A<S> {
        return null;
    }
}
var a = new A<number>();
A.B(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 2,
        },
    );
    f.insert(t, "a");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B(v: A<number>): A<number>".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 2,
        },
    );
    f.insert(t, "); A.B(");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B(v: A<unknown>): A<unknown>".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 2,
        },
    );
    f.insert(t, "a");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B(v: A<number>): A<number>".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 2,
        },
    );
    done();
}
