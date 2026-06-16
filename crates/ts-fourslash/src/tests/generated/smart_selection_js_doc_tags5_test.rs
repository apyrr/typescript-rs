#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_js_doc_tags5() {
    let mut t = TestingT;
    run_test_smart_selection_js_doc_tags5(&mut t);
}

fn run_test_smart_selection_js_doc_tags5(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_JSDocTags5") {
        return;
    }
    let content = r"/**
 * @callback Foo
 * @param {string} data
 * @param {/**/number} [index] - comment
 * @return {boolean}
 */

/** @type {Foo} */
const foo = s => !(s.length % 2);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
