#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_require_in_ts3() {
    let mut t = TestingT;
    run_test_code_fix_require_in_ts3(&mut t);
}

fn run_test_code_fix_require_in_ts3(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixRequireInTs3") {
        return;
    }
    let content = r#"// @Filename: /a.ts
const { a, b: { c } } = [|require("a")|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
