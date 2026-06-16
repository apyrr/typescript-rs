#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_array_literal() {
    let mut t = TestingT;
    run_test_formatting_array_literal(&mut t);
}

fn run_test_formatting_array_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/x= [];
y = [
/*2*/           1,
/*3*/  2
/*4*/ ];

z = [[
/*5*/  1,
/*6*/             2
/*7*/      ]  ];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "x = [];");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    1,");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    2");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "];");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    1,");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    2");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "]];");
    done();
}
