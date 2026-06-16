#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures13() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures13(&mut t);
}

fn run_test_js_doc_function_signatures13(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures13") {
        return;
    }
    let content = r"/**
 * @template {string} K/**/ a golden opportunity
 */
function Multimap(iv) {
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "any", "");
    done();
}
