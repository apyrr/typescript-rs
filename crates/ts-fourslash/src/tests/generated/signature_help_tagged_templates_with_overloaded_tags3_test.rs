#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_tagged_templates_with_overloaded_tags3() {
    let mut t = TestingT;
    run_test_signature_help_tagged_templates_with_overloaded_tags3(&mut t);
}

fn run_test_signature_help_tagged_templates_with_overloaded_tags3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function f(templateStrings: TemplateStringsArray, p1_o1: string): number;
function f(templateStrings: TemplateStringsArray, p1_o2: number, p2_o2: number, p3_o2: number): string;
function f(templateStrings: TemplateStringsArray, p1_o3: string, p2_o3: boolean, p3_o3: number): boolean;
function f(...foo[]: any) { return ""; }

f ` + "`" + `${/*1*/ "s/*2*/tring" /*3*/ }   ${"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.go_to_marker(t, &marker);
        f.verify_signature_help_options(t, VerifySignatureHelpOptions {
    text: Some("f(templateStrings: TemplateStringsArray, p1_o3: string, p2_o3: boolean, p3_o3: number): boolean".to_string()),
    parameter_name: Some("p1_o3".to_string()),
    parameter_span: Some("p1_o3: string".to_string()),
    parameter_count: Some(4),
    overloads_count: 3,
});
    }
    done();
}
