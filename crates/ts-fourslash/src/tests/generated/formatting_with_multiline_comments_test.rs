#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_with_multiline_comments() {
    let mut t = TestingT;
    run_test_formatting_with_multiline_comments(&mut t);
}

fn run_test_formatting_with_multiline_comments(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingWithMultilineComments") {
        return;
    }
    let content = r"f(/*
/*2*/         */() => { /*1*/ });";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "         */() => {");
    done();
}
