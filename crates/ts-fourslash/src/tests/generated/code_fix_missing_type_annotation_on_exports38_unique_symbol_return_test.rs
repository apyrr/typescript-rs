#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports38_unique_symbol_return() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports38_unique_symbol_return(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports38_unique_symbol_return(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
// @Filename: /code.ts
const u: unique symbol = Symbol();
export const fn = () => ({ u } as const);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type '{ readonly u: typeof u; }'".to_string(),
            new_file_content: r"const u: unique symbol = Symbol();
export const fn = (): {
    readonly u: typeof u;
} => ({ u } as const);"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
