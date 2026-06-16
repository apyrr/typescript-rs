#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_with_enter_after_multiline_string() {
    let mut t = TestingT;
    run_test_formatting_with_enter_after_multiline_string(&mut t);
}

fn run_test_formatting_with_enter_after_multiline_string(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingWithEnterAfterMultilineString") {
        return;
    }
    let content = r#"class Greeter3 {
    stop() {
        /*2*/var s = "hello\
"/*1*/
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "        var s = \"hello\\");
    done();
}
