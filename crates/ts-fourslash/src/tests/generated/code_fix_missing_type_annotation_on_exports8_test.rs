#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports8() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports8(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports8(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports8") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
function foo() {return 42;}
export const g = function () { return foo(); };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(t, Some(&vec!["Add return type 'number'".to_string()]));
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'number'".to_string(),
            new_file_content: r"function foo() {return 42;}
export const g = function (): number { return foo(); };"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
