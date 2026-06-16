#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_function_overload_count() {
    let mut t = TestingT;
    run_test_function_overload_count(&mut t);
}

fn run_test_function_overload_count(t: &mut TestingT) {
    if should_skip_if_failing("TestFunctionOverloadCount") {
        return;
    }
    let content = r#"class C1 {
    public attr(): string;
    public attr(i: number): string;
    public attr(i: number, x: boolean): string;
    public attr(i?: any, x?: any) {
        return "hi";
    }
}
var i = new C1;
i.attr(/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 3,
        },
    );
    done();
}
