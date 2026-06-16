#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_member8() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_member8(&mut t);
}

fn run_test_code_fix_add_missing_member8(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAddMissingMember8") {
        return;
    }
    let content = r"// @Filename: a.ts
declare var x: [1, 2];
x.b;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
