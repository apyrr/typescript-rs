#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_expressions_in_if_condition() {
    let mut t = TestingT;
    run_test_formatting_expressions_in_if_condition(&mut t);
}

fn run_test_formatting_expressions_in_if_condition(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingExpressionsInIfCondition") {
        return;
    }
    let content = r"if (a === 1 ||
    /*0*/b === 2 ||/*1*/
    c === 3) {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.go_to_marker(t, "0");
    f.verify_current_line_content(t, "    b === 2 ||");
    done();
}
