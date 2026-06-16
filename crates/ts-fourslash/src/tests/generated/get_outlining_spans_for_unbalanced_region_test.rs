#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_spans_for_unbalanced_region() {
    let mut t = TestingT;
    run_test_get_outlining_spans_for_unbalanced_region(&mut t);
}

fn run_test_get_outlining_spans_for_unbalanced_region(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningSpansForUnbalancedRegion") {
        return;
    }
    let content = r"// top-heavy region balance
// #region unmatched

[|// #region matched

// #endregion matched|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
