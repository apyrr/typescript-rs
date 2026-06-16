#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_js_doc_type_def() {
    let mut t = TestingT;
    run_test_find_all_refs_js_doc_type_def(&mut t);
}

fn run_test_find_all_refs_js_doc_type_def(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsJsDocTypeDef") {
        return;
    }
    let content = r"/** @typedef {Object} /*0*/T */
function foo() {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string()]);
    done();
}
