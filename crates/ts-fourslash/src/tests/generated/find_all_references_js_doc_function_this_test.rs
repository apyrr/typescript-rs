#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_js_doc_function_this() {
    let mut t = TestingT;
    run_test_find_all_references_js_doc_function_this(&mut t);
}

fn run_test_find_all_references_js_doc_function_this(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllReferencesJSDocFunctionThis") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: Foo.js
/** @type {function (this: string, string): string} */
var f = function (s) { return /*0*/this + s; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string()]);
    done();
}
