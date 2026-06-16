#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_disable_js_diagnostics_in_file() {
    let mut t = TestingT;
    run_test_code_fix_disable_js_diagnostics_in_file(&mut t);
}

fn run_test_code_fix_disable_js_diagnostics_in_file(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowjs: true
// @noEmit: true
// @Filename: a.js
[|// @ts-check|]
var x = "";
x = 1;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "// @ts-nocheck", false, 0, 1);
    done();
}
