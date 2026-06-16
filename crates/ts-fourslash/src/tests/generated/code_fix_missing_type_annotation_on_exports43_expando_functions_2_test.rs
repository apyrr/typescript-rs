#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports43_expando_functions_2() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports43_expando_functions_2(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports43_expando_functions_2(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports43-expando-functions-2") {
        return;
    }
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
// @Filename: /code.ts
const foo = () => {}
foo/*a*/["a"] = "A";
foo["b"] = "C""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add annotation of type '{ (): void; a: string; b: string; }'".to_string(),
            new_file_content: r#"const foo: {
    (): void;
    a: string;
    b: string;
} = () => {}
foo["a"] = "A";
foo["b"] = "C""#
                .to_string(),
            new_range_content: String::new(),
            index: 1,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
