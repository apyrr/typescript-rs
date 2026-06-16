#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_js_doc_tags13() {
    let mut t = TestingT;
    run_test_smart_selection_js_doc_tags13(&mut t);
}

fn run_test_smart_selection_js_doc_tags13(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_JSDocTags13") {
        return;
    }
    let content = r"let a;
let b: {
    /** Comment */ /*1*/p0: number
    /** Comment */ /*2*/p1: number
    /** Comment */ /*3*/p2: number
};
let c;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
