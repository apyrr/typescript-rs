#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navbar_nested_common_js_exports() {
    let mut t = TestingT;
    run_test_navbar_nested_common_js_exports(&mut t);
}

fn run_test_navbar_nested_common_js_exports(t: &mut TestingT) {
    if should_skip_if_failing("TestNavbarNestedCommonJsExports") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
exports.a = exports.b = exports.c = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
