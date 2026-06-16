#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_overloaded_function_parameter() {
    let mut t = TestingT;
    run_test_rename_js_overloaded_function_parameter(&mut t);
}

fn run_test_rename_js_overloaded_function_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJsOverloadedFunctionParameter") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: foo.js
/**
 * @overload
 * @param {number} x
 * @returns {number}
 *
 * @overload
 * @param {string} x
 * @returns {string} 
 *
 * @param {unknown} x
 * @returns {unknown} 
 */
function foo(x/**/) {
  return x;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["".to_string()]);
    done();
}
