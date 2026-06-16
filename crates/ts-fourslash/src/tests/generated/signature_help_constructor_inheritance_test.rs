#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_constructor_inheritance() {
    let mut t = TestingT;
    run_test_signature_help_constructor_inheritance(&mut t);
}

fn run_test_signature_help_constructor_inheritance(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpConstructorInheritance") {
        return;
    }
    let content = r"class base {
    constructor(s: string);
    constructor(n: number);
    constructor(a: any) { }
}
class B1 extends base { }
class B2 extends B1 { }
class B3 extends B2 {
    constructor() {
        super(/*indirectSuperCall*/3);
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "indirectSuperCall");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("B2(n: number): B2".to_string()),
            parameter_name: Some("n".to_string()),
            parameter_span: Some("n: number".to_string()),
            parameter_count: Some(1),
            overloads_count: 2,
        },
    );
    done();
}
