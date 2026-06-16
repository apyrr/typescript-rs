#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_nested_do_while_by_enter() {
    let mut t = TestingT;
    run_test_formatting_on_nested_do_while_by_enter(&mut t);
}

fn run_test_formatting_on_nested_do_while_by_enter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*2*/do{
/*3*/do/*1*/{
/*4*/do{
/*5*/}while(a!==b)
/*6*/}while(a!==b)
/*7*/}while(a!==b)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_current_line_content(t, "    {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "do{");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    do");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "do{");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "}while(a!==b)");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "}while(a!==b)");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "}while(a!==b)");
    done();
}
