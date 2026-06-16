#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports32_inline_short_hand() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports32_inline_short_hand(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports32_inline_short_hand(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports32-inline-short-hand") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @Filename: /code.ts
const x = 1;
export default {
  x
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add satisfies and an inline type assertion with 'number'".to_string(),
            new_file_content: r"const x = 1;
export default {
  x: x as number
};"
            .to_string(),
            new_range_content: String::new(),
            index: 1,
            apply_changes: false,
            user_preferences: None,
        },
    );
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add satisfies and an inline type assertion with 'typeof x'".to_string(),
            new_file_content: r"const x = 1;
export default {
  x: x as typeof x
};"
            .to_string(),
            new_range_content: String::new(),
            index: 2,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
