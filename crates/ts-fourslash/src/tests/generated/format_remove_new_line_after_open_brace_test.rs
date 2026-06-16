#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_remove_new_line_after_open_brace() {
    let mut t = TestingT;
    run_test_format_remove_new_line_after_open_brace(&mut t);
}

fn run_test_format_remove_new_line_after_open_brace(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatRemoveNewLineAfterOpenBrace") {
        return;
    }
    let content = r"function foo()
{
}
if (true)
{
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"function foo() {
}
if (true) {
}",
    );
    done();
}
