#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports36_conditional_releative() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports36_conditional_releative(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports36_conditional_releative(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports36-conditional-releative") {
        return;
    }
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @Filename: /code.ts
const A = "A"
const B = "B"
export const AB = Math.random()? A: B;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(
        t,
        Some(&vec![
            "Add annotation of type '\"A\" | \"B\"'".to_string(),
            "Add annotation of type 'typeof A | typeof B'".to_string(),
            "Add annotation of type 'string'".to_string(),
            "Add satisfies and an inline type assertion with '\"A\" | \"B\"'".to_string(),
            "Add satisfies and an inline type assertion with 'typeof A | typeof B'".to_string(),
            "Add satisfies and an inline type assertion with 'string'".to_string(),
        ]),
    );
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add satisfies and an inline type assertion with 'typeof A | typeof B'"
                .to_string(),
            new_file_content: r#"const A = "A"
const B = "B"
export const AB = (Math.random() ? A : B) satisfies typeof A | typeof B as typeof A | typeof B;"#
                .to_string(),
            new_range_content: String::new(),
            index: 4,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
