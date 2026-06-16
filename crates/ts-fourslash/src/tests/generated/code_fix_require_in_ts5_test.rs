#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_require_in_ts5() {
    let mut t = TestingT;
    run_test_code_fix_require_in_ts5(&mut t);
}

fn run_test_code_fix_require_in_ts5(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixRequireInTs5") {
        return;
    }
    let content = r"// @Filename: /a.ts
const a = 1;
const b = 2;
const foo = require(`foo${a}${b}`);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
