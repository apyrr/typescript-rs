#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_skipped_tokens() {
    let mut t = TestingT;
    run_test_formatting_skipped_tokens(&mut t);
}

fn run_test_formatting_skipped_tokens(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingSkippedTokens") {
        return;
    }
    let content = r"/*1*/foo(): Bar { }
/*2*/function Foo      () #   { }
/*3*/4+:5
 namespace M {
function a(
/*4*/    : T) { }
}
/*5*/var x       =";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "foo(): Bar { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "function Foo() #   { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "4 +: 5");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    : T) { }");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "var x =");
    done();
}
