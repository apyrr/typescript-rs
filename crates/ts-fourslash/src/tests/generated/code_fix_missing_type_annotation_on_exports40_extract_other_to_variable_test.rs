#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports40_extract_other_to_variable() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports40_extract_other_to_variable(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports40_extract_other_to_variable(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
// @Filename: /code.ts
let c: string[] = [];
export let o = {
    p: Math.random() ? []: [
        ...c
    ]
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Extract to variable and replace with 'newLocal as typeof newLocal'"
                .to_string(),
            new_file_content: r"let c: string[] = [];
const newLocal = Math.random() ? [] : [
    ...c
];
export let o = {
    p: newLocal as typeof newLocal
}"
            .to_string(),
            new_range_content: String::new(),
            index: 2,
            apply_changes: true,
            user_preferences: None,
        },
    );
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add annotation of type 'string[]'".to_string(),
            new_file_content: r"let c: string[] = [];
const newLocal: string[] = Math.random() ? [] : [
    ...c
];
export let o = {
    p: newLocal as typeof newLocal
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: true,
            user_preferences: None,
        },
    );
    done();
}
