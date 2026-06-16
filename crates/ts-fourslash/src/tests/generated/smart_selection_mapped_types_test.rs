#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_selection_mapped_types() {
    let mut t = TestingT;
    run_test_smart_selection_mapped_types(&mut t);
}

fn run_test_smart_selection_mapped_types(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartSelection_mappedTypes") {
        return;
    }
    let content = r"type M = { /*1*/-re/*2*/adonly /*3*/[K in ke/*4*/yof any]/*5*/-/*6*/?: any };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_selection_ranges(t, &[]);
    done();
}
