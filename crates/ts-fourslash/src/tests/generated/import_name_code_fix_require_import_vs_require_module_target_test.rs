#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_require_import_vs_require_module_target() {
    let mut t = TestingT;
    run_test_import_name_code_fix_require_import_vs_require_module_target(&mut t);
}

fn run_test_import_name_code_fix_require_import_vs_require_module_target(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @checkJs: true
// @module: es2015
// @Filename: a.js
export const x = 0;
// @Filename: index.js
x";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "index.js");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add import from \"./a\"".to_string(),
            new_file_content: r#"import { x } from "./a";

x"#
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    f.go_to_position(t, 0);
    f.insert_line(t, "const fs = require('fs');\n");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add import from \"./a\"".to_string(),
            new_file_content: r"const fs = require('fs');
const { x } = require('./a');

x"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
