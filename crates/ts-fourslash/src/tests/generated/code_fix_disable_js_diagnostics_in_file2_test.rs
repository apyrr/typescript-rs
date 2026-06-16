#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_disable_js_diagnostics_in_file2() {
    let mut t = TestingT;
    run_test_code_fix_disable_js_diagnostics_in_file2(&mut t);
}

fn run_test_code_fix_disable_js_diagnostics_in_file2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowjs: true
// @noEmit: true
// @checkJs: true
// @Filename: a.js
[|var x = "";
x = 1;|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "// @ts-nocheck\nvar x = \"\";\nx = 1;", false, 0, 1);
    done();
}
