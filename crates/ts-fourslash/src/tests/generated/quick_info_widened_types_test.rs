#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_widened_types() {
    let mut t = TestingT;
    run_test_quick_info_widened_types(&mut t);
}

fn run_test_quick_info_widened_types(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoWidenedTypes") {
        return;
    }
    let content = r"// @strict: false
var /*1*/a = null;                   // var a: any
var /*2*/b = undefined;              // var b: any
var /*3*/c = { x: 0, y: null };	// var c: { x: number, y: any }
var /*4*/d = [null, undefined];      // var d: any[]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var a: any", "");
    f.verify_quick_info_at(t, "2", "var b: any", "");
    f.verify_quick_info_at(t, "3", "var c: {\n    x: number;\n    y: any;\n}", "");
    f.verify_quick_info_at(t, "4", "var d: any[]", "");
    done();
}
