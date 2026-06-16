#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_infer_from_usage_variable3_js() {
    let mut t = TestingT;
    run_test_code_fix_infer_from_usage_variable3_js(&mut t);
}

fn run_test_code_fix_infer_from_usage_variable3_js(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixInferFromUsageVariable3JS") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @noEmit: true
// @noImplicitAny: false
// @Filename: important.js
[|function f(foo) {
    foo += 2
    return foo
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "/** \n * @param {number} foo\n */\nfunction f(foo) {\n    foo += 2\n    return foo\n}\n",
        false,
        0,
        0,
    );
    done();
}
