#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_export_as_default_existing_import() {
    let mut t = TestingT;
    run_test_import_name_code_fix_export_as_default_existing_import(&mut t);
}

fn run_test_import_name_code_fix_export_as_default_existing_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import [|{ v1, v2, v3 }|] from "./module";
v4/*0*/();
// @Filename: module.ts
const v4 = 5;
export { v4 as default };
export const v1 = 5;
export const v2 = 5;
export const v3 = 5;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(t, &vec![r"v4, { v1, v2, v3 }".to_string()], None);
    done();
}
