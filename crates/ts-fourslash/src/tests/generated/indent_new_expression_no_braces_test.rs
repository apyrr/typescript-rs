#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_indent_new_expression_no_braces() {
    let mut t = TestingT;
    run_test_indent_new_expression_no_braces(&mut t);
}

fn run_test_indent_new_expression_no_braces(t: &mut TestingT) {
    if should_skip_if_failing("TestIndentNewExpressionNoBraces") {
        return;
    }
    let content = r"new Foo/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 0);
    done();
}
