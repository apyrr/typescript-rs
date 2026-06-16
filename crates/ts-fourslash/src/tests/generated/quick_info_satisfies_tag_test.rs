#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_satisfies_tag() {
    let mut t = TestingT;
    run_test_quick_info_satisfies_tag(&mut t);
}

fn run_test_quick_info_satisfies_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoSatisfiesTag") {
        return;
    }
    let content = r"// @noEmit: true
// @allowJS: true
// @checkJs: true
// @filename: /a.js
/** @satisfies {number} comment */
const /*1*/a = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
