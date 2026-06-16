#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_class() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_class(&mut t);
}

fn run_test_quick_info_display_parts_class(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsClass") {
        return;
    }
    let content = r"class /*1*/c {
}
var /*2*/cInstance = new /*3*/c();
var /*4*/cVal = /*5*/c;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
