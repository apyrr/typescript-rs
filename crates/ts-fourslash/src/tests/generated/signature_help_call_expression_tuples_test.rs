#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_call_expression_tuples() {
    let mut t = TestingT;
    run_test_signature_help_call_expression_tuples(&mut t);
}

fn run_test_signature_help_call_expression_tuples(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpCallExpressionTuples") {
        return;
    }
    let content = r"function fnTest(str: string, num: number) { }
declare function wrap<A extends any[], R>(fn: (...a: A) => R) : (...a: A) => R;
var fnWrapped = wrap(fnTest);
fnWrapped/*3*/(/*1*/'', /*2*/5);
function fnTestVariadic (str: string, ...num: number[]) { }
var fnVariadicWrapped = wrap(fnTestVariadic);
fnVariadicWrapped/*4*/(/*5*/'', /*6*/5);
function fnNoParams () { }
var fnNoParamsWrapped = wrap(fnNoParams);
fnNoParamsWrapped/*7*/(/*8*/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "3",
        "var fnWrapped: (str: string, num: number) => void",
        "",
    );
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("fnWrapped(str: string, num: number): void".to_string()),
            parameter_name: Some("str".to_string()),
            parameter_span: Some("str: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("num".to_string()),
            parameter_span: Some("num: number".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_quick_info_at(
        t,
        "4",
        "var fnVariadicWrapped: (str: string, ...num: number[]) => void",
        "",
    );
    f.go_to_marker(t, "5");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("fnVariadicWrapped(str: string, ...num: number[]): void".to_string()),
            parameter_name: Some("str".to_string()),
            parameter_span: Some("str: string".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "6");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("num".to_string()),
            parameter_span: Some("...num: number[]".to_string()),
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_quick_info_at(t, "7", "var fnNoParamsWrapped: () => void", "");
    f.go_to_marker(t, "8");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("fnNoParamsWrapped(): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: Some(0),
            overloads_count: 0,
        },
    );
    done();
}
