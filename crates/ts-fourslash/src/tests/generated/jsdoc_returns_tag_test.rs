#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_returns_tag() {
    let mut t = TestingT;
    run_test_jsdoc_returns_tag(&mut t);
}

fn run_test_jsdoc_returns_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocReturnsTag") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: dummy.js
/**
 * Find an item
 * @template T
 * @param {T[]} l
 * @param {T} x
 * @returns {?T}  The names of the found item(s).
 */
function find(l, x) {
}
find(''/**/);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
