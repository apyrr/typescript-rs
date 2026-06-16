#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_js_doc_type_literal() {
    let mut t = TestingT;
    run_test_find_all_references_js_doc_type_literal(&mut t);
}

fn run_test_find_all_references_js_doc_type_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesJsDocTypeLiteral") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: foo.js
/**
 * @param {object} o - very important!
 * @param {string} o.x - a thing, its ok
 * @param {number} o.y - another thing
 * @param {Object} o.nested - very nested
 * @param {boolean} o.nested./*1*/great - much greatness
 * @param {number} o.nested.times - twice? probably!??
 */
 function f(o) { return o.nested./*2*/great; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
