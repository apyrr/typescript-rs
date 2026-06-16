#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_class_extends_interface_sig_help1() {
    let mut t = TestingT;
    run_test_class_extends_interface_sig_help1(&mut t);
}

fn run_test_class_extends_interface_sig_help1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    public foo(x: string);
    public foo(x: number);
    public foo(x: any) { return x; }
}
interface I extends C {
    other(x: any): any;
}
var i: I;
i.foo(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: Some("x: string".to_string()),
            parameter_count: None,
            overloads_count: 2,
        },
    );
    done();
}
