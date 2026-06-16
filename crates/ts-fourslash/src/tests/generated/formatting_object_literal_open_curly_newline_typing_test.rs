#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_object_literal_open_curly_newline_typing() {
    let mut t = TestingT;
    run_test_formatting_object_literal_open_curly_newline_typing(&mut t);
}

fn run_test_formatting_object_literal_open_curly_newline_typing(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingObjectLiteralOpenCurlyNewlineTyping") {
        return;
    }
    let content = r"
var varName =/**/
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "\n{");
    f.verify_current_file_content(
        t,
        r"
var varName =
    {
",
    );
    f.insert(t, "\na: 1");
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
var varName =
{
    a: 1
",
    );
    f.insert(t, "\n};");
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
var varName =
{
    a: 1
};
",
    );
    done();
}
