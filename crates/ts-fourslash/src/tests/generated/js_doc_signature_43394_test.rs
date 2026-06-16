#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_signature_43394() {
    let mut t = TestingT;
    run_test_js_doc_signature_43394(&mut t);
}

fn run_test_js_doc_signature_43394(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocSignature-43394") {
        return;
    }
    let content = r"/**
 * @typedef {Object} Foo
 * @property {number} ...
 * /**/@typedef {number} Bar
 */";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
