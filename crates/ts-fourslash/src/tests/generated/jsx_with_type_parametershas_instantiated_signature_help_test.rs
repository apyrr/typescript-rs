#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsx_with_type_parametershas_instantiated_signature_help() {
    let mut t = TestingT;
    run_test_jsx_with_type_parametershas_instantiated_signature_help(&mut t);
}

fn run_test_jsx_with_type_parametershas_instantiated_signature_help(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare namespace JSX {
    interface Element {
        render(): Element | string | false;
    }
}

function SFC<T>(_props: Record<string, T>) {
    return '';
}

(</*1*/SFC/>);
(</*2*/SFC<string>/>);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("SFC(_props: Record<string, unknown>): string".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("SFC(_props: Record<string, string>): string".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
