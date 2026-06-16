#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_in_function_type_reference2() {
    let mut t = TestingT;
    run_test_quick_info_in_function_type_reference2(&mut t);
}

fn run_test_quick_info_in_function_type_reference2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoInFunctionTypeReference2") {
        return;
    }
    let content = r"class C<T> {
    map(fn: (/*1*/k: string, /*2*/value: T, context: any) => void, context: any) {
    }
}
var c: C<number>;
c.map(/*3*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) k: string", "");
    f.verify_quick_info_at(t, "2", "(parameter) value: T", "");
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some(
                "map(fn: (k: string, value: number, context: any) => void, context: any): void"
                    .to_string(),
            ),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
