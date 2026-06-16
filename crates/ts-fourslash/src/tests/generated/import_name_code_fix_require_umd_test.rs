#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_require_umd() {
    let mut t = TestingT;
    run_test_import_name_code_fix_require_umd(&mut t);
}

fn run_test_import_name_code_fix_require_umd(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_require_UMD") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @module: commonjs
// @esModuleInterop: false
// @allowSyntheticDefaultImports: false
// @Filename: umd.d.ts
namespace Foo { function f() {} }
export = Foo;
export as namespace Foo;
// @Filename: index.js
Foo;
module.exports = {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "index.js");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add import from \"./umd\"".to_string(),
            new_file_content: r#"const Foo = require("./umd");

Foo;
module.exports = {};"#
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
