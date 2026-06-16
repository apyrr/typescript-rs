#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_all_js() {
    let mut t = TestingT;
    run_test_import_name_code_fix_all_js(&mut t);
}

fn run_test_import_name_code_fix_all_js(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_all_js") {
        return;
    }
    let content = r"// @module: esnext
// @allowJs: true
// @checkJs: true
// @Filename: /a.js
export class C {}
/** @typedef {number} T */
// @Filename: /b.js
C;
/** @type {T} */
const x = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.js");
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingImport".to_string(),
            new_file_content: r#"import { C } from "./a";

C;
/** @type {import("./a").T} */
const x = 0;"#
                .to_string(),
        },
    );
    done();
}
