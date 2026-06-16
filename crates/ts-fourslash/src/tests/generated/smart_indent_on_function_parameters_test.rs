#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_on_function_parameters() {
    let mut t = TestingT;
    run_test_smart_indent_on_function_parameters(&mut t);
}

fn run_test_smart_indent_on_function_parameters(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo(a,
        /*2*/b,/*0*/
             //ABC/*3*/
    /*4*/c
    ) {
};
var x = [
    /*5*///DEF/*1*/
    1,/*6*/
        2/*7*/
]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "0");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    b,");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "             //ABC");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    c");
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    //DEF");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    1,");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        2");
    done();
}
