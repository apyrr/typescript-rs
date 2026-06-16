#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_disable_js_diagnostics_in_file6() {
    let mut t = TestingT;
    run_test_code_fix_disable_js_diagnostics_in_file6(&mut t);
}

fn run_test_code_fix_disable_js_diagnostics_in_file6(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
// @allowjs: true
// @noEmit: true
// @checkJs: true
// @Filename: a.js
var x = 0;

function f(_a) {[|
    f(x());
|]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "    // @ts-ignore\n    f(x());", false, 0, 0);
    done();
}
