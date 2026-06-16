#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_indent_after_function_closing_braces() {
    let mut t = TestingT;
    run_test_indent_after_function_closing_braces(&mut t);
}

fn run_test_indent_after_function_closing_braces(t: &mut TestingT) {
    if should_skip_if_failing("TestIndentAfterFunctionClosingBraces") {
        return;
    }
    let content = r"class foo {
    public f() {
        return 0;
    /*1*/}/*2*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "2");
    f.insert_line(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    }");
    done();
}
