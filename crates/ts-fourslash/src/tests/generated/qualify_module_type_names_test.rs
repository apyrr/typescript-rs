#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_qualify_module_type_names() {
    let mut t = TestingT;
    run_test_qualify_module_type_names(&mut t);
}

fn run_test_qualify_module_type_names(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace m { export class c { } };
function x(arg: m.c) { return arg; }
x(/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("x(arg: m.c): m.c".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
