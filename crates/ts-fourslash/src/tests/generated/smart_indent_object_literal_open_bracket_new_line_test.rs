#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_object_literal_open_bracket_new_line() {
    let mut t = TestingT;
    run_test_smart_indent_object_literal_open_bracket_new_line(&mut t);
}

fn run_test_smart_indent_object_literal_open_bracket_new_line(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentObjectLiteralOpenBracketNewLine") {
        return;
    }
    let content = r"var a =/*1*/
    {/*2*/}

var b = {
    outer:/*3*/
           {/*4*/}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 0);
    f.go_to_marker(t, "2");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "3");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "4");
    f.insert(t, "\n");
    f.verify_indentation(t, 8);
    done();
}
