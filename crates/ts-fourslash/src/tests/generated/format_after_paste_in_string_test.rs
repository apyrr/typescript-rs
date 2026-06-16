#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_after_paste_in_string() {
    let mut t = TestingT;
    run_test_format_after_paste_in_string(&mut t);
}

fn run_test_format_after_paste_in_string(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*2*/const x = f('aa/*1*/a').x()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.paste(t, "bb");
    f.format_document(t, "");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "const x = f('aabba').x()");
    done();
}
