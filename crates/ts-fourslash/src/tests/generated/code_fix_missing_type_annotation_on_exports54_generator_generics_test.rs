#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports54_generator_generics() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports54_generator_generics(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports54_generator_generics(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports54-generator-generics") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2015
export function foo(x: Generator<number>) {
    return x;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'Generator<number>'".to_string(),
            new_file_content: r"export function foo(x: Generator<number>): Generator<number> {
    return x;
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
