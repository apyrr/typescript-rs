#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_new_import_file3() {
    let mut t = TestingT;
    run_test_import_name_code_fix_new_import_file3(&mut t);
}

fn run_test_import_name_code_fix_new_import_file3(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFixNewImportFile3") {
        return;
    }
    let content = r"[|let t: XXX/*0*/.I;|]
// @Filename: ./module.ts
export namespace XXX {
   export interface I {
   }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import { XXX } from "./module";

let t: XXX.I;"#
                .to_string(),
        ],
        None,
    );
    done();
}
