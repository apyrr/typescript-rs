#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling5() {
    let mut t = TestingT;
    run_test_code_fix_spelling5(&mut t);
}

fn run_test_code_fix_spelling5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: f1.ts
export const fooooooooo = 1;
// @Filename: f2.ts
import {[|fooooooooa|]} from "./f1"; fooooooooa;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "f2.ts");
    f.verify_range_after_code_fix(t, "fooooooooo", false, 0, 0);
    done();
}
