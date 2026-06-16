#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports55_generator_return() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports55_generator_return(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports55_generator_return(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports55-generator-return") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2015
export function *foo() {
    yield 5;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'Generator<number, void, unknown>'".to_string(),
            new_file_content: r"export function *foo(): Generator<number, void, unknown> {
    yield 5;
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
