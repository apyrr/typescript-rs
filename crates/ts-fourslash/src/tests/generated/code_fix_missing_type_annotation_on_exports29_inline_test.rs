#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports29_inline() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports29_inline(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports29_inline(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
function getString() {
    return ""
}
export const exp = {
    prop: getString()
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add satisfies and an inline type assertion with 'string'".to_string(),
            new_file_content: r#"function getString() {
    return ""
}
export const exp = {
    prop: getString() satisfies string as string
};"#
            .to_string(),
            new_range_content: String::new(),
            index: 1,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
