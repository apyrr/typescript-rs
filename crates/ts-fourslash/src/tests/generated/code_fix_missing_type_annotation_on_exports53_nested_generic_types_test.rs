#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports53_nested_generic_types() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports53_nested_generic_types(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports53_nested_generic_types(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports53-nested-generic-types") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
export interface Foo<T, U = T[]> {}
export function foo(x: Map<number, Foo<string>>) {
    return x;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'Map<number, Foo<string, string[]>>'".to_string(),
            new_file_content: r"export interface Foo<T, U = T[]> {}
export function foo(x: Map<number, Foo<string>>): Map<number, Foo<string, string[]>> {
    return x;
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
