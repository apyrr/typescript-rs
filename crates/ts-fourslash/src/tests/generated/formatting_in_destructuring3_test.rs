#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_in_destructuring3() {
    let mut t = TestingT;
    run_test_formatting_in_destructuring3(&mut t);
}

fn run_test_formatting_in_destructuring3(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingInDestructuring3") {
        return;
    }
    let content = r"/*1*/const {
/*2*/    a,
/*3*/    b,
/*4*/} = {a: 1, b: 2};
/*5*/const {a: c} = {a: 1, b: 2};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "const {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    a,");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    b,");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "} = { a: 1, b: 2 };");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "const { a: c } = { a: 1, b: 2 };");
    done();
}
