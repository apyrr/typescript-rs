#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags10() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags10(&mut t);
}

fn run_test_quick_info_js_doc_tags10(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags10") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJs: true
// @Filename: quickInfoJsDocTags10.js
/**
 * @param {T1} a
 * @param {T2} a
 * @template T1,T2 Comment Text
 */
const /**/foo = (a, b) => {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
