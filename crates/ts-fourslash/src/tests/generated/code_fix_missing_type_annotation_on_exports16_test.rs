#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports16() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports16(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports16(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports16") {
        return;
    }
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
function foo() {
    return { x: 1, y: {42: {dd: "45"}, b: 2} };
}
function foo3(): "42" {
    return "42";
}
export const { x: a , y: { [foo3()]: {dd: e} } } = foo();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Extract binding expressions to variable".to_string(),
            new_file_content: r#"function foo() {
    return { x: 1, y: {42: {dd: "45"}, b: 2} };
}
function foo3(): "42" {
    return "42";
}
const dest = foo();
export const a: number = dest.x;
const _a = foo3();
export const e: string = (dest.y)[_a].dd;"#
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
