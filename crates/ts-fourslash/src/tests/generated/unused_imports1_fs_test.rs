#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_imports1_fs() {
    let mut t = TestingT;
    run_test_unused_imports1_fs(&mut t);
}

fn run_test_unused_imports1_fs(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedImports1FS") {
        return;
    }
    let content = r#"// @noUnusedLocals: true
// @Filename: file2.ts
  [|import { Calculator } from "./file1" |]
// @Filename: file1.ts
   export class Calculator {

   }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "", false, 0, 0);
    done();
}
