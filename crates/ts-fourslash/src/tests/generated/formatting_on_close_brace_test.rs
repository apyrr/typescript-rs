#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_close_brace() {
    let mut t = TestingT;
    run_test_formatting_on_close_brace(&mut t);
}

fn run_test_formatting_on_close_brace(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnCloseBrace") {
        return;
    }
    let content = r"class foo    {
    /**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "}");
    f.go_to_bof(t);
    f.verify_current_line_content(t, "class foo {");
    done();
}
