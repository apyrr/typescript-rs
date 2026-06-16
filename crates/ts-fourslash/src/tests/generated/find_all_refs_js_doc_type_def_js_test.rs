#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_js_doc_type_def_js() {
    let mut t = TestingT;
    run_test_find_all_refs_js_doc_type_def_js(&mut t);
}

fn run_test_find_all_refs_js_doc_type_def_js(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsJsDocTypeDef_js") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
/** /*1*/@typedef {number} /*2*/T */

/**
 * @return {/*3*/T}
 */
function f(obj) { return 0; }

/**
 * @return {/*4*/T}
 */
function f2(obj) { return 0; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
