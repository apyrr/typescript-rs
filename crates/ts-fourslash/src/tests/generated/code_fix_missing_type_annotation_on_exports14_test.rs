#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports14() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports14(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports14(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
function foo() {
    return { x: 1, y: 1};
}
export const { x, y = 0} = foo(), z= 42;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Extract binding expressions to variable".to_string(),
            new_file_content: r"function foo() {
    return { x: 1, y: 1};
}
const dest = foo();
export const x: number = dest.x;
const temp = dest.y;
export const y: number = temp === undefined ? 0 : dest.y;
export const z = 42;"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
