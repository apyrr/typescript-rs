#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_existing_import7() {
    let mut t = TestingT;
    run_test_import_name_code_fix_existing_import7(&mut t);
}

fn run_test_import_name_code_fix_existing_import7(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import [|{ v1 }|] from "../other_dir/module";
f1/*0*/();
// @Filename: ../other_dir/module.ts
export var v1 = 5;
export function f1();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(t, &vec![r"{ f1, v1 }".to_string()], None);
    done();
}
