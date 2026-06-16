#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_existing_import3() {
    let mut t = TestingT;
    run_test_import_name_code_fix_existing_import3(&mut t);
}

fn run_test_import_name_code_fix_existing_import3(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixExistingImport3") {
        return;
    }
    let content = r#"[|import d, * as ns from "./module"   ;
f1/*0*/();|]
// @Filename: module.ts
export function f1() {}
export var v1 = 5;
export default var d1 = 6;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import d, * as ns from "./module"   ;
ns.f1();"#
                .to_string(),
            r#"import d, * as ns from "./module"   ;
import { f1 } from "./module";
f1();"#
                .to_string(),
        ],
        None,
    );
    done();
}
