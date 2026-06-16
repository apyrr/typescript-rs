#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports50_generics_with_default() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports50_generics_with_default(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports50_generics_with_default(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2015
let x: Iterator<number>;
export const y = x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add annotation of type 'Iterator<number>'".to_string(),
            new_file_content: r"let x: Iterator<number>;
export const y: Iterator<number> = x;"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
