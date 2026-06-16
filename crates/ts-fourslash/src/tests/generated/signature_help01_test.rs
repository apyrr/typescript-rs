#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help01() {
    let mut t = TestingT;
    run_test_signature_help01(&mut t);
}

fn run_test_signature_help01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
function foo(data: number) {
}

function bar {
    foo(/*1*/)
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(1),
            overloads_count: 0,
        },
    );
    done();
}
