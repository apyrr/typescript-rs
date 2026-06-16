#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures7() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures7(&mut t);
}

fn run_test_js_doc_function_signatures7(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures7") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: Foo.js
/**
 * @param {string} p0
 * @param {string} [p1]
 */
function Test(p0, p1) {
    this.P0 = p0;
    this.P1 = p1;
}


var /**/test = new Test("");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "var test: Test", "");
    done();
}
