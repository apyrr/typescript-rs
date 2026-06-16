#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_typedef_quick_info1() {
    let mut t = TestingT;
    run_test_js_doc_typedef_quick_info1(&mut t);
}

fn run_test_js_doc_typedef_quick_info1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocTypedefQuickInfo1") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: jsDocTypedef1.js
/**
 * @typedef {Object} Opts
 * @property {string} x
 * @property {string=} y
 * @property {string} [z]
 * @property {string} [w="hi"]
 * 
 * @param {Opts} opts
 */
function foo(/*1*/opts) {
    opts.x;
}
foo({x: 'abc'});
/**
 * @typedef {object} Opts1
 * @property {string} x
 * @property {string=} y
 * @property {string} [z]
 * @property {string} [w="hi"]
 * 
 * @param {Opts1} opts
 */
function foo1(/*2*/opts1) {
    opts1.x;
}
foo1({x: 'abc'});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
