#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_class_default_named() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_class_default_named(&mut t);
}

fn run_test_quick_info_display_parts_class_default_named(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsClassDefaultNamed") {
        return;
    }
    let content = r"/*1*/export /*2*/default /*3*/class /*4*/C /*5*/ {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
