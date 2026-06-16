#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_else_inside_a_function() {
    let mut t = TestingT;
    run_test_formatting_else_inside_a_function(&mut t);
}

fn run_test_formatting_else_inside_a_function(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingElseInsideAFunction") {
        return;
    }
    let content = r"var x = function() {
    if (true) {
    /*1*/} else {/*2*/
}

// newline at the end of the file";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "2");
    f.insert_line(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    } else {");
    done();
}
