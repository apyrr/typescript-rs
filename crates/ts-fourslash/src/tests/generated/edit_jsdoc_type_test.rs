#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_edit_jsdoc_type() {
    let mut t = TestingT;
    run_test_edit_jsdoc_type(&mut t);
}

fn run_test_edit_jsdoc_type(t: &mut TestingT) {
    if should_skip_if_failing("TestEditJsdocType") {
        return;
    }
    let content = r"// @allowJs: true
// @noLib: true
// @Filename: /a.js
/** @type/**/ */
const x = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "", "");
    f.insert(t, " ");
    f.verify_quick_info_is(t, "", "");
    done();
}
