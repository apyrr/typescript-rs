#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_nested_export_equal_export_default() {
    let mut t = TestingT;
    run_test_quick_info_nested_export_equal_export_default(&mut t);
}

fn run_test_quick_info_nested_export_equal_export_default(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoNestedExportEqualExportDefault") {
        return;
    }
    let content = r"export = (state, messages) => {
   export/*1*/ default/*2*/ {
   }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
