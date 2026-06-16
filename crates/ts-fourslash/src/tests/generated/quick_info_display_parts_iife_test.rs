#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_iife() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_iife(&mut t);
}

fn run_test_quick_info_display_parts_iife(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsIife") {
        return;
    }
    let content = r"// @strictNullChecks: true
var iife = (function foo/*1*/(x, y) { return x })(12);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(local function) foo(x: number, y?: undefined): number",
        "",
    );
    done();
}
