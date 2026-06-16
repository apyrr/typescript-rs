#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports26_fn_in_object_literal() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports26_fn_in_object_literal(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports26_fn_in_object_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
export const extensions = {
    /**
     */
    fn: <T>(actualValue: T, expectedValue: T) => {
       return actualValue === expectedValue
    },
    fn2: function<T>(actualValue: T, expectedValue: T)  {
       return actualValue === expectedValue
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingTypeAnnotationOnExports".to_string(),
            new_file_content: r"export const extensions = {
    /**
     */
    fn: <T>(actualValue: T, expectedValue: T): boolean => {
       return actualValue === expectedValue
    },
    fn2: function<T>(actualValue: T, expectedValue: T): boolean  {
       return actualValue === expectedValue
    }
}"
            .to_string(),
        },
    );
    done();
}
