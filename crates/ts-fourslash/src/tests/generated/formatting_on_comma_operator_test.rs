#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_comma_operator() {
    let mut t = TestingT;
    run_test_formatting_on_comma_operator(&mut t);
}

fn run_test_formatting_on_comma_operator(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnCommaOperator") {
        return;
    }
    let content = r"var v1 = ((1, 2, 3), 4, 5, (6, 7));/*1*/
function f1() {
    var a = 1;
    return a, v1, a;/*2*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "var v1 = ((1, 2, 3), 4, 5, (6, 7));");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    return a, v1, a;");
    done();
}
