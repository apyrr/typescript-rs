#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports21_params_and_return() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports21_params_and_return(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports21_params_and_return(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports21-params-and-return") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
/**
 * Test
 */
export function foo(): number { return 0; }
/**
* Docs
*/
export const bar = (a = foo()) =>
   a;
// Trivia";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'number'".to_string(),
            new_file_content: r"/**
 * Test
 */
export function foo(): number { return 0; }
/**
* Docs
*/
export const bar = (a = foo()): number =>
   a;
// Trivia"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: true,
            user_preferences: None,
        },
    );
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add annotation of type 'number'".to_string(),
            new_file_content: r"/**
 * Test
 */
export function foo(): number { return 0; }
/**
* Docs
*/
export const bar = (a: number = foo()): number =>
   a;
// Trivia"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: true,
            user_preferences: None,
        },
    );
    done();
}
