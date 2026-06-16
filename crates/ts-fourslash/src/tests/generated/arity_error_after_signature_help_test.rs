#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_arity_error_after_signature_help() {
    let mut t = TestingT;
    run_test_arity_error_after_signature_help(&mut t);
}

fn run_test_arity_error_after_signature_help(t: &mut TestingT) {
    if should_skip_if_failing("TestArityErrorAfterSignatureHelp") {
        return;
    }
    let content = r"// @strict: true

declare function f(x: string, y: number): any;

/*1*/f/*2*/(/*3*/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.insert(t, "\"");
    f.insert(t, "\"");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_code_fix_not_available(t, &[]);
    f.verify_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("2"));
    done();
}
