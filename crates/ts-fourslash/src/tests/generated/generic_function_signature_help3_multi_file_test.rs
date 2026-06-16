#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_function_signature_help3_multi_file() {
    let mut t = TestingT;
    run_test_generic_function_signature_help3_multi_file(&mut t);
}

fn run_test_generic_function_signature_help3_multi_file(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericFunctionSignatureHelp3MultiFile") {
        return;
    }
    let content = r"// @Filename: genericFunctionSignatureHelp_0.ts
function foo1<T>(x: number, callback: (y1: T) => number) { }
// @Filename: genericFunctionSignatureHelp_1.ts
function foo2<T>(x: number, callback: (y2: T) => number) { }
// @Filename: genericFunctionSignatureHelp_2.ts
function foo3<T>(x: number, callback: (y3: T) => number) { }
// @Filename: genericFunctionSignatureHelp_3.ts
function foo4<T>(x: number, callback: (y4: T) => number) { }
// @Filename: genericFunctionSignatureHelp_4.ts
function foo5<T>(x: number, callback: (y5: T) => number) { }
// @Filename: genericFunctionSignatureHelp_5.ts
function foo6<T>(x: number, callback: (y6: T) => number) { }
// @Filename: genericFunctionSignatureHelp_6.ts
function foo7<T>(x: number, callback: (y7: T) => number) { }
// @Filename: genericFunctionSignatureHelp_7.ts
foo1(/*1*/               // signature help shows y as T
foo2(1,/*2*/             // signature help shows y as {}
foo3(1, (/*3*/           // signature help shows y as T
foo4<string>(1,/*4*/     // signature help shows y as string
foo5<string>(1, (/*5*/   // signature help shows y as T
foo6(1, </*6*/           // signature help shows y as {}
foo7(1, <string>(/*7*/   // signature help shows y as T";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo1(x: number, callback: (y1: unknown) => number): void".to_string()),
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
            text: Some("foo2(x: number, callback: (y2: unknown) => number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("callback(y3: unknown): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "4");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo4(x: number, callback: (y4: string) => number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "5");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("callback(y5: string): number".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.go_to_marker(t, "6");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo6(x: number, callback: (y6: unknown) => number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.insert(t, "string>(null,null);");
    f.go_to_marker(t, "7");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo7(x: number, callback: (y7: unknown) => number): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
