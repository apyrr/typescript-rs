#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_jsdoc_typedef_missing_type() {
    let mut t = TestingT;
    run_test_quick_info_jsdoc_typedef_missing_type(&mut t);
}

fn run_test_quick_info_jsdoc_typedef_missing_type(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsdocTypedefMissingType") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
/**
 * @typedef /**/A
 */
var x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "type A = any", "");
    done();
}
