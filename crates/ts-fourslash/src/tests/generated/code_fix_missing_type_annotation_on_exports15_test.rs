#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports15() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports15(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports15(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports15") {
        return;
    }
    let content = r"// @stableTypeOrdering: true
// @isolatedDeclarations: true
// @declaration: true
function foo() {
    return { x: 1, y: 1 } as const;
}
export const { x, y = 0 } = foo();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Extract binding expressions to variable".to_string(),
            new_file_content: r"function foo() {
    return { x: 1, y: 1 } as const;
}
const dest = foo();
export const x: 1 = dest.x;
const temp = dest.y;
export const y: 0 | 1 = temp === undefined ? 0 : dest.y;"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
