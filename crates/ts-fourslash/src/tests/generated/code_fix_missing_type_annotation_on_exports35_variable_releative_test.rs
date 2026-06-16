#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports35_variable_releative() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports35_variable_releative(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports35_variable_releative(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports35-variable-releative") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @Filename: /code.ts
const foo = { a: 1 }
export const exported = foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add annotation of type 'typeof foo'".to_string(),
            new_file_content: r"const foo = { a: 1 }
export const exported: typeof foo = foo;"
                .to_string(),
            new_range_content: String::new(),
            index: 1,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
