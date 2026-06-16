#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_js_doc_tags4() {
    let mut t = TestingT;
    run_test_smart_selection_js_doc_tags4(&mut t);
}

fn run_test_smart_selection_js_doc_tags4(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_JSDocTags4") {
        return;
    }
    let content = r"/**
 * @typedef {object} Foo
 * @property {string} a
 * @property {number} b
 * @property {/**/number} c
 */

/** @type {Foo} */
const foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
