#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_multiline_comment_before_open_brace() {
    let mut t = TestingT;
    run_test_multiline_comment_before_open_brace(&mut t);
}

fn run_test_multiline_comment_before_open_brace(t: &mut TestingT) {
    if should_skip_if_failing("TestMultilineCommentBeforeOpenBrace") {
        return;
    }
    let content = r"function test() /*1*//* %^ */
{
    if (true) /*2*//* %^ */
    {
    }
}
function a() {
    /* %^ */ }/*3*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "function test() /* %^ */ {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    if (true) /* %^ */ {");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "}");
    done();
}
