#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports_type_predicate1() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports_type_predicate1(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports_type_predicate1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @filename: index.ts
export function isString(value: unknown) {
  return typeof value === "string";
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'value is string'".to_string(),
            new_file_content: r#"export function isString(value: unknown): value is string {
  return typeof value === "string";
}"#
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
