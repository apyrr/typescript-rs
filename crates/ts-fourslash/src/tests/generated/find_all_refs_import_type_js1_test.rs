#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_import_type_js1() {
    let mut t = TestingT;
    run_test_find_all_refs_import_type_js1(&mut t);
}

fn run_test_find_all_refs_import_type_js1(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefs_importType_js1") {
        return;
    }
    let content = r#"// @allowJs: true
// @checkJs: true
// @Filename: /a.js
module.exports = class /**/C {};
module.exports.D = class D {};
// @Filename: /b.js
/** @type {import("./a")} */
const x = 0;
/** @type {import("./a").D} */
const y = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
