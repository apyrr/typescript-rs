#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_satisfies_tag_rename() {
    let mut t = TestingT;
    run_test_jsdoc_satisfies_tag_rename(&mut t);
}

fn run_test_jsdoc_satisfies_tag_rename(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocSatisfiesTagRename") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJS: true
// @checkJs: true
// @filename: /a.js
/**
 * @typedef {Object} T
 * @property {number} a
 */

/** @satisfies {/**/T} comment */
const foo = { a: 1 };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
