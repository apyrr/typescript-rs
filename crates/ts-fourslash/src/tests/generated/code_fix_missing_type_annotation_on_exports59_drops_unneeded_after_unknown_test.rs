#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports59_drops_unneeded_after_unknown() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports59_drops_unneeded_after_unknown(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports59_drops_unneeded_after_unknown(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true

export interface Foo<S = string, T = unknown, U = number> {}
export function g(x: Foo<number, unknown, number>) { return x; }
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'Foo<number>'".to_string(),
            new_file_content: r"
export interface Foo<S = string, T = unknown, U = number> {}
export function g(x: Foo<number, unknown, number>): Foo<number> { return x; }
"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
