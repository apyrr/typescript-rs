#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags11() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags11(&mut t);
}

fn run_test_quick_info_js_doc_tags11(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags11") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @Filename: quickInfoJsDocTags11.js
/**
 * @param {T1} a
 * @param {T2} b
 * @template {number} T1 Comment T1
 * @template {number} T2 Comment T2
 */
const /**/foo = (a, b) => {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
