#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_do_while_no_semicolon() {
    let mut t = TestingT;
    run_test_formatting_on_do_while_no_semicolon(&mut t);
}

fn run_test_formatting_on_do_while_no_semicolon(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnDoWhileNoSemicolon") {
        return;
    }
    let content = r"/*2*/do {
/*3*/    for (var i = 0; i < 10; i++)
/*4*/        i -= 2
/*5*/        }/*1*/while (1 !== 1)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_current_line_content(t, "while (1 !== 1)");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "do {");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    for (var i = 0; i < 10; i++)");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "        i -= 2");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "}");
    done();
}
