#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags_typedef() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags_typedef(&mut t);
}

fn run_test_quick_info_js_doc_tags_typedef(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTagsTypedef") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @Filename: quickInfoJsDocTagsTypedef.js
/**
 * Bar comment
 * @typedef {Object} /*1*/Bar
 * @property {string} baz - baz comment
 * @property {string} qux - qux comment
 */

/**
 * foo comment
 * @param {/*2*/Bar} x - x comment
 * @returns {Bar}
 */
function foo(x) {
    return x;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
