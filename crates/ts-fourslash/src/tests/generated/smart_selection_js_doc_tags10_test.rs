#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_js_doc_tags10() {
    let mut t = TestingT;
    run_test_smart_selection_js_doc_tags10(&mut t);
}

fn run_test_smart_selection_js_doc_tags10(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_JSDocTags10") {
        return;
    }
    let content = r"/**
 * @template T
 * @extends {/**/Set<T>}
 */
class A extends B {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
