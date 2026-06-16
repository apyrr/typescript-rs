#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_add_missing_function_declaration16() {
    let mut t = TestingT;
    run_test_code_fix_add_missing_function_declaration16(&mut t);
}

fn run_test_code_fix_add_missing_function_declaration16(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAddMissingFunctionDeclaration16") {
        return;
    }
    let content = r#"// @moduleResolution: bundler
// @filename: /node_modules/test/index.js
export const x = 1;
// @filename: /foo.ts
import * as test from "test";
test.foo();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/foo.ts");
    f.verify_code_fix_not_available(t, &[]);
    done();
}
