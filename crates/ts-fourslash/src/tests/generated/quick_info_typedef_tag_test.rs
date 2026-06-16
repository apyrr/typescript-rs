#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_typedef_tag() {
    let mut t = TestingT;
    run_test_quick_info_typedef_tag(&mut t);
}

fn run_test_quick_info_typedef_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoTypedefTag") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: a.js
/**
 * The typedef tag should not appear in the quickinfo.
 * @typedef {{ foo: 'foo' }} Foo
 */
function f() { }
f/*1*/()
/**
 * A removed comment
 * @tag Usage shows that non-param tags in comments explain the typedef instead of using it
 * @typedef {{ nope: any }} Nope not here
 * @tag comment 2
 */
function g() { }
g/*2*/()
/**
 * The whole thing is kept
 * @param {Local} keep
 * @typedef {{ local: any }} Local kept too
 * @returns {void} also kept
 */
function h(keep) { }
h/*3*/({ nope: 1 })";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
