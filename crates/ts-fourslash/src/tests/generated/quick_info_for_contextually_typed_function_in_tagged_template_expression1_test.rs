#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_contextually_typed_function_in_tagged_template_expression1() {
    let mut t = TestingT;
    run_test_quick_info_for_contextually_typed_function_in_tagged_template_expression1(&mut t);
}

fn run_test_quick_info_for_contextually_typed_function_in_tagged_template_expression1(
    t: &mut TestingT,
) {
    if should_skip_if_failing(
        "TestQuickInfoForContextuallyTypedFunctionInTaggedTemplateExpression1",
    ) {
        return;
    }
    let content = r"function tempTag1<T>(templateStrs: TemplateStringsArray, f: (x: T) => T, x: T): T;
function tempTag1<T>(templateStrs: TemplateStringsArray, f: (x: T) => T, h: (y: T) => T, x: T): T;
function tempTag1<T>(...rest: any[]): T {
    return undefined;
}

tempTag1 `${ x => /*0*/x }${ 10 }`;
tempTag1 `${ x => /*1*/x }${ x => /*2*/x }${ 10 }`;
tempTag1 `${ x => /*3*/x }${ (x: number) => /*4*/x }${ undefined }`;
tempTag1 `${ (x: number) => /*5*/x }${ x => /*6*/x }${ undefined }`;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for marker in f.marker_names() {
        f.verify_quick_info_at(t, &marker, "(parameter) x: number", "");
    }
    done();
}
