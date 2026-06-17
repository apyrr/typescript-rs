#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_import_type() {
    let mut t = TestingT;
    run_test_import_name_code_fix_import_type(&mut t);
}

fn run_test_import_name_code_fix_import_type(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_importType") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: /a.js
export {};
/** @typedef {number} T */
// @Filename: /b.js
/** @type {T} */
const x = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.js");
    f.verify_import_fix_at_position(
        t,
        &vec![r#"/** @type {import("./a").T} */
const x = 0;"#
            .to_string()],
        None,
    );
    done();
}
