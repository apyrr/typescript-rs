#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_js_doc_unknown_tag() {
    let mut t = TestingT;
    run_test_quick_info_for_js_doc_unknown_tag(&mut t);
}

fn run_test_quick_info_for_js_doc_unknown_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForJSDocUnknownTag") {
        return;
    }
    let content = r"/**
 * @example
 * if (true) {
 *     foo()
 * }
 */
function fo/*1*/o() {
    return '2';
}
/**
 @example
 {
     foo()
 }
 */
function fo/*2*/o2() {
    return '2';
}
/**
 * @example
 *   x y
 *   12345
 *      b
 */
function m/*3*/oo() {
    return '2';
}
/**
 * @func
 * @example
 *   x y
 *   12345
 *      b
 */
function b/*4*/oo() {
    return '2';
}
/**
 * @func
 * @example    x y
 *             12345
 *                b
 */
function go/*5*/o() {
    return '2';
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
