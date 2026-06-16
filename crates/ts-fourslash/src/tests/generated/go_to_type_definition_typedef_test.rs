#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition_typedef() {
    let mut t = TestingT;
    run_test_go_to_type_definition_typedef(&mut t);
}

fn run_test_go_to_type_definition_typedef(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToTypeDefinition_typedef") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
/**
 * /*def*/@typedef {object} I
 * @property {number} x
 */

/** @type {I} */
const /*ref*/i = { x: 0 };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(t, &["ref".to_string()]);
    done();
}
