#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_js_doc() {
    let mut t = TestingT;
    run_test_smart_selection_js_doc(&mut t);
}

fn run_test_smart_selection_js_doc(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_JSDoc") {
        return;
    }
    let content = r"// Not a JSDoc comment
/**
 * @param {number} x The number to square
 */
function /**/square(x) {
  return x * x;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
