#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_js_doc_with_unresolved_http_links() {
    let mut t = TestingT;
    run_test_quick_info_for_js_doc_with_unresolved_http_links(&mut t);
}

fn run_test_quick_info_for_js_doc_with_unresolved_http_links(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForJSDocWithUnresolvedHttpLinks") {
        return;
    }
    let content = r"// @checkJs: true
// @filename: quickInfoForJSDocWithHttpLinks.js
/** @see {@link https://hva} */
var /*5*/see2 = true

/** {@link https://hvaD} */
var /*6*/see3 = true";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
