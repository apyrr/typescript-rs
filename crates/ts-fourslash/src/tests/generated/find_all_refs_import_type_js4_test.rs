#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_type_js4() {
    let mut t = TestingT;
    run_test_find_all_refs_import_type_js4(&mut t);
}

fn run_test_find_all_refs_import_type_js4(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefs_importType_js4") {
        return;
    }
    let content = r#"// @module: commonjs
// @allowJs: true
// @checkJs: true
// @Filename: /a.js
/**
 * @callback /**/A
 * @param {unknown} response
 */

module.exports = {};
// @Filename: /b.js
/** @typedef {import("./a").A} A */"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
