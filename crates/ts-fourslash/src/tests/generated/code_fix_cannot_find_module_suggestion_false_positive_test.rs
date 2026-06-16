#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_cannot_find_module_suggestion_false_positive() {
    let mut t = TestingT;
    run_test_code_fix_cannot_find_module_suggestion_false_positive(&mut t);
}

fn run_test_code_fix_cannot_find_module_suggestion_false_positive(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixCannotFindModule_suggestion_falsePositive") {
        return;
    }
    let content = r#"// @moduleResolution: bundler
// @module: commonjs
// @resolveJsonModule: true
// @strict: true
// @Filename: /node_modules/foo/bar.json
{ "a": 0 }
// @Filename: /a.ts
import abs = require([|"foo/bar.json"|]);
abs;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_file(t, "/a.ts");
    f.verify_suggestion_diagnostics(&[]);
    done();
}
