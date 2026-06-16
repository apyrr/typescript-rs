#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_js_doc_comment_with_no_tags() {
    let mut t = TestingT;
    run_test_navigation_bar_js_doc_comment_with_no_tags(&mut t);
}

fn run_test_navigation_bar_js_doc_comment_with_no_tags(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarJsDocCommentWithNoTags") {
        return;
    }
    let content = r"/** Test */
export const Test = {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
