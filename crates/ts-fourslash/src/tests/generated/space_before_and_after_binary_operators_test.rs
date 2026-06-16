#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_space_before_and_after_binary_operators() {
    let mut t = TestingT;
    run_test_space_before_and_after_binary_operators(&mut t);
}

fn run_test_space_before_and_after_binary_operators(t: &mut TestingT) {
    if should_skip_if_failing("TestSpaceBeforeAndAfterBinaryOperators") {
        return;
    }
    let content = r"let i = 0;
/*1*/(i++,i++);
/*2*/(i++,++i);
/*3*/(1,2);
/*4*/(i++,2);
/*5*/(i++,i++,++i,i--,2);
let s = 'foo';
/*6*/for (var i = 0,ii = 2; i < s.length; ii++,i++) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "(i++, i++);");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "(i++, ++i);");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "(1, 2);");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "(i++, 2);");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "(i++, i++, ++i, i--, 2);");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "for (var i = 0, ii = 2; i < s.length; ii++, i++) {");
    done();
}
