#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_existing_import11() {
    let mut t = TestingT;
    run_test_import_name_code_fix_existing_import11(&mut t);
}

fn run_test_import_name_code_fix_existing_import11(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixExistingImport11") {
        return;
    }
    let content = r#"import [|{
    v1, v2,
    v3
}|] from "./module";
f1/*0*/();
// @Filename: module.ts
 export function f1() {}
 export var v1 = 5;
 export var v2 = 5;
 export var v3 = 5;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r"{
    f1,
    v1, v2,
    v3
}"
            .to_string(),
        ],
        None,
    );
    done();
}
