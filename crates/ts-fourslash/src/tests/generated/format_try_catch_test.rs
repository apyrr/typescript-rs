#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_try_catch() {
    let mut t = TestingT;
    run_test_format_try_catch(&mut t);
}

fn run_test_format_try_catch(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function test() {
    /*try*/try {
    }
    /*catch*/catch (e) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.format_document(t, "");
    f.format_document(t, "");
    f.go_to_marker(t, "try");
    f.verify_current_line_content(t, "    try {");
    f.go_to_marker(t, "catch");
    f.verify_current_line_content(t, "    catch (e) {");
    done();
}
