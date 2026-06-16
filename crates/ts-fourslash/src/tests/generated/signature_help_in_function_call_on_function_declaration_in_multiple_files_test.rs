#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_in_function_call_on_function_declaration_in_multiple_files() {
    let mut t = TestingT;
    run_test_signature_help_in_function_call_on_function_declaration_in_multiple_files(&mut t);
}

fn run_test_signature_help_in_function_call_on_function_declaration_in_multiple_files(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"// @Filename: signatureHelpInFunctionCallOnFunctionDeclarationInMultipleFiles_file0.ts
declare function fn(x: string, y: number);
// @Filename: signatureHelpInFunctionCallOnFunctionDeclarationInMultipleFiles_file1.ts
declare function fn(x: string);
// @Filename: signatureHelpInFunctionCallOnFunctionDeclarationInMultipleFiles_file2.ts
fn(/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 2,
        },
    );
    done();
}
