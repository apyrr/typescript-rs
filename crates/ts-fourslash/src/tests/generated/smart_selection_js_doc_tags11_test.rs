#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_js_doc_tags11() {
    let mut t = TestingT;
    run_test_smart_selection_js_doc_tags11(&mut t);
}

fn run_test_smart_selection_js_doc_tags11(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_JSDocTags11") {
        return;
    }
    let content = r"const x = 1;
type Foo = {
  /** comment */
  /*2*/readonly /*1*/status: number;
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
