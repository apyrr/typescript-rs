#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_import_nonunicode_path() {
    let mut t = TestingT;
    run_test_quick_info_import_nonunicode_path(&mut t);
}

fn run_test_quick_info_import_nonunicode_path(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoImportNonunicodePath") {
        return;
    }
    let content = r#"// @Filename: /江南今何在/tmp.ts
export const foo = 1;
// @Filename: /test.ts
import { foo } from "./江南/*1*/今何在/tmp";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "module \"./江南今何在/tmp\"", "");
    done();
}
