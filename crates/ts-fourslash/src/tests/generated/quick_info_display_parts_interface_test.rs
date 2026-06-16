#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_interface() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_interface(&mut t);
}

fn run_test_quick_info_display_parts_interface(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsInterface") {
        return;
    }
    let content = r"interface /*1*/i {
}
var /*2*/iInstance: /*3*/i;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
