#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags14() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags14(&mut t);
}

fn run_test_quick_info_js_doc_tags14(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags14") {
        return;
    }
    let content = r"/**
 * @param {Object} options the args object
 * @param {number} options.a first number
 * @param {number} options.b second number
 * @param {Object} options.c sub-object
 * @param {number} options.c.d third number
 * @param {Function} callback the callback function
 * @returns {number}
 */
function /**/fn(options, callback = null) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
