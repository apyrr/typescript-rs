#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags_callback() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags_callback(&mut t);
}

fn run_test_quick_info_js_doc_tags_callback(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTagsCallback") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @Filename: quickInfoJsDocTagsCallback.js
/**
 * @callback cb/*1*/
 * @param {string} x - x comment
 */

/**
 * @param {/*2*/cb} bar -callback comment
 */
function foo(bar) {
    bar(bar);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
