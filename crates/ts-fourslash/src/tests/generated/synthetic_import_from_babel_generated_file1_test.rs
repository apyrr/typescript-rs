#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_synthetic_import_from_babel_generated_file1() {
    let mut t = TestingT;
    run_test_synthetic_import_from_babel_generated_file1(&mut t);
}

fn run_test_synthetic_import_from_babel_generated_file1(t: &mut TestingT) {
    if should_skip_if_failing("TestSyntheticImportFromBabelGeneratedFile1") {
        return;
    }
    let content = r#"// @allowJs: true
// @allowSyntheticDefaultImports: true
// @Filename: /a.js
exports.__esModule = true;
exports.default = f;
/**
 * Run this function
 * @param {string} t
 */
function f(t) {}
// @Filename: /b.js
import f from "./a"
/**/f"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "",
        "(alias) function f(t: string): void\nimport f",
        "Run this function",
    );
    done();
}
