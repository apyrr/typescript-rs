#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_convert_to_unknown_for_non_overlapping_types9() {
    let mut t = TestingT;
    run_test_code_fix_add_convert_to_unknown_for_non_overlapping_types9(&mut t);
}

fn run_test_code_fix_add_convert_to_unknown_for_non_overlapping_types9(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAddConvertToUnknownForNonOverlappingTypes9") {
        return;
    }
    let content = r"// @checkJs: true
// @allowJs: true
// @filename: a.js
let x = /** @type {string} */ (100);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(
        t,
        &vec!["Add 'unknown' conversion for non-overlapping types".to_string()],
    );
    done();
}
