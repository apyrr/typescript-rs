#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_js_doc_tags1() {
    let mut t = TestingT;
    run_test_smart_selection_js_doc_tags1(&mut t);
}

fn run_test_smart_selection_js_doc_tags1(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_JSDocTags1") {
        return;
    }
    let content = r"/**
 * @returns {Array<{ value: /**/string }>}
 */
function foo() { return [] }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
