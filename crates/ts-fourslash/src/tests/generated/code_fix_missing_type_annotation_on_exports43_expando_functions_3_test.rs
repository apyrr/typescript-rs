#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports43_expando_functions_3() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports43_expando_functions_3(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports43_expando_functions_3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
// @Filename: /code.ts
function foo(): void {}
foo.x = 1;
foo.y = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Annotate types of properties expando function in a namespace".to_string(),
            new_file_content: r"function foo(): void {}
declare namespace foo {
    export var x: number;
    export var y: number;
}
foo.x = 1;
foo.y = 1;"
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
