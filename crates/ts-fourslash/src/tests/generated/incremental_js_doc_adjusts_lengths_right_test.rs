#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_js_doc_adjusts_lengths_right() {
    let mut t = TestingT;
    run_test_incremental_js_doc_adjusts_lengths_right(&mut t);
}

fn run_test_incremental_js_doc_adjusts_lengths_right(t: &mut TestingT) {
    if should_skip_if_failing("TestIncrementalJsDocAdjustsLengthsRight") {
        return;
    }
    let content = r"// @noLib: true

/**
 * Pad `str` to `width`.
 *
 * @param {String} str
 * @param {Number} wid/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "th\n@");
    done();
}
