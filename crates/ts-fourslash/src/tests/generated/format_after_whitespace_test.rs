#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_after_whitespace() {
    let mut t = TestingT;
    run_test_format_after_whitespace(&mut t);
}

fn run_test_format_after_whitespace(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatAfterWhitespace") {
        return;
    }
    let content = r"function foo()
{
    var bar;
    /*1*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.verify_current_file_content(
        t,
        r"function foo()
{
    var bar;


}",
    );
    done();
}
