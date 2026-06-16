#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports44_default_export() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports44_default_export(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports44_default_export(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports44-default-export") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
// @Filename: /code.ts
export default 1 + 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Extract default export to variable".to_string(),
            new_file_content: r"const _default_1: number = 1 + 1;
export default _default_1;"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
