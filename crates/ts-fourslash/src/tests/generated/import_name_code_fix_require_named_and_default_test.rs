#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_require_named_and_default() {
    let mut t = TestingT;
    run_test_import_name_code_fix_require_named_and_default(&mut t);
}

fn run_test_import_name_code_fix_require_named_and_default(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_require_namedAndDefault") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: blah.ts
export default class Blah {}
export const Named1 = 0;
export const Named2 = 1;
// @Filename: index.js
Named1 + Named2;
new Blah;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "index.js");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"const { default: Blah, Named1, Named2 } = require("./blah");

Named1 + Named2;
new Blah;"#
                .to_string(),
        },
    );
    done();
}
