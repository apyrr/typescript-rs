#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports57_generics_doesnt_drop_trailing_unknown() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports57_generics_doesnt_drop_trailing_unknown(
        &mut t,
    );
}

fn run_test_code_fix_missing_type_annotation_on_exports57_generics_doesnt_drop_trailing_unknown(
    t: &mut TestingT,
) {
    if should_skip_if_failing(
        "TestCodeFixMissingTypeAnnotationOnExports57-generics-doesnt-drop-trailing-unknown",
    ) {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2015

let x: unknown;
export const s = new Set([x]);
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add annotation of type 'Set<unknown>'".to_string(),
            new_file_content: r"
let x: unknown;
export const s: Set<unknown> = new Set([x]);
"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
