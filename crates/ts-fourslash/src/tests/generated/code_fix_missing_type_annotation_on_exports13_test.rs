#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports13() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports13(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports13(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports13") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
function foo() {
    return { x: 1, y: 1 };
}
export const { x: abcd, y: defg } = foo();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Extract binding expressions to variable".to_string(),
            new_file_content: r"function foo() {
    return { x: 1, y: 1 };
}
const dest = foo();
export const abcd: number = dest.x;
export const defg: number = dest.y;"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
