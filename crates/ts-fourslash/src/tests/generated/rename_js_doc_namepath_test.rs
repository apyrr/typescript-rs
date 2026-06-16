#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_js_doc_namepath() {
    let mut t = TestingT;
    run_test_rename_js_doc_namepath(&mut t);
}

fn run_test_rename_js_doc_namepath(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameJSDocNamepath") {
        return;
    }
    let content = r"// @noLib: true
/**
 * @type {module:foo/A} x
 */
var x = 1
var /*0*/A = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename(t, &["0".to_string()]);
    done();
}
