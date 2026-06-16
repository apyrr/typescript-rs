#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_property_tag() {
    let mut t = TestingT;
    run_test_quick_info_property_tag(&mut t);
}

fn run_test_quick_info_property_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoPropertyTag") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
/**
 * @typedef I
 * @property {number} x Doc
 *                      More doc
 */

/** @type {I} */
const obj = { /**/x: 10 };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(property) x: number", "Doc\nMore doc");
    done();
}
