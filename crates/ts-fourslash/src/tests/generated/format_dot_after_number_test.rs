#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_dot_after_number() {
    let mut t = TestingT;
    run_test_format_dot_after_number(&mut t);
}

fn run_test_format_dot_after_number(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"1+ 2 .toString() +3/*1*/
1+ 2. .toString() +3/*2*/
1+ 2.0 .toString() +3/*3*/
1+ (2) .toString() +3/*4*/
1+ 2_000 .toString() +3/*5*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "1 + 2 .toString() + 3");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "1 + 2..toString() + 3");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "1 + 2.0.toString() + 3");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "1 + (2).toString() + 3");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "1 + 2_000 .toString() + 3");
    done();
}
