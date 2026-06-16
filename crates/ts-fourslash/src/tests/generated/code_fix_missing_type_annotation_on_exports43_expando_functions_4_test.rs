#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports43_expando_functions_4() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports43_expando_functions_4(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports43_expando_functions_4(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports43-expando-functions-4") {
        return;
    }
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
// @Filename: /code.ts
function foo(): void {}
// cannot name this property because it's an invalid variable name.
foo["@bar"] = 42;
foo.x = 1;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Annotate types of properties expando function in a namespace".to_string(),
            new_file_content: r#"function foo(): void {}
declare namespace foo {
    export var x: number;
}
// cannot name this property because it's an invalid variable name.
foo["@bar"] = 42;
foo.x = 1;"#
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
