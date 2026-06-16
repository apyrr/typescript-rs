#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_function_new() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_function_new(&mut t);
}

fn run_test_quick_info_js_doc_function_new(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJSDocFunctionNew") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: Foo.js
/** @type {function (new: string, string): string} */
var f/**/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "var f: new (arg1: string) => string", "");
    done();
}
