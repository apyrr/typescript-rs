#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_jsdoc_enum() {
    let mut t = TestingT;
    run_test_quick_info_jsdoc_enum(&mut t);
}

fn run_test_quick_info_jsdoc_enum(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsdocEnum") {
        return;
    }
    let content = r"// @allowJs: true
// @noLib: true
// @Filename: /a.js
/**
 * Doc
 * @enum {number}
 */
const E = {
    A: 0,
}

/** @type {/*type*/E} */
const x = /*value*/E.A;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_quick_info_at(t, "type", "type E = number", "Doc");
    f.verify_quick_info_at(t, "value", "const E: {\n    A: number;\n}", "Doc");
    done();
}
