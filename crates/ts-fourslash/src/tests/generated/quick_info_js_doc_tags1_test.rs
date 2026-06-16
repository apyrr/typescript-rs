#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags1() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags1(&mut t);
}

fn run_test_quick_info_js_doc_tags1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTags1") {
        return;
    }
    let content = r"// @Filename: quickInfoJsDocTags1.ts
/**
 * Doc
 * @author Me <me@domain.tld>
 * @augments {C<T>} Augments it
 * @template T A template
 * @type {number | string} A type
 * @typedef {number | string} NumOrStr
 * @property {number} x The prop
 * @param {number} x The param
 * @returns The result
 * @see x (the parameter)
 */
function /**/foo(x) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
