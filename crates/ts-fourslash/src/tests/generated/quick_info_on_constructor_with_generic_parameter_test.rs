#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_constructor_with_generic_parameter() {
    let mut t = TestingT;
    run_test_quick_info_on_constructor_with_generic_parameter(&mut t);
}

fn run_test_quick_info_on_constructor_with_generic_parameter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    x: number;
}
class Foo<T> {
    y: T;
}
class A {
    foo() { }
}
class B extends A {
    constructor(a: Foo<I>, b: number) {
        super();
    }
}
var x = new /*2*/B(/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B(a: Foo<I>, b: number): B".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.insert(t, "null,");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B(a: Foo<I>, b: number): B".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.insert(t, "10);");
    f.verify_quick_info_at(t, "2", "constructor B(a: Foo<I>, b: number): B", "");
    done();
}
