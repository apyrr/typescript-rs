#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports41_no_computed_enum_members() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports41_no_computed_enum_members(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports41_no_computed_enum_members(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @lib: es2019
// @Filename: /code.ts
enum E {
    A = "foo".length
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
