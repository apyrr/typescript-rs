#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_comments_before_errors() {
    let mut t = TestingT;
    run_test_formatting_comments_before_errors(&mut t);
}

fn run_test_formatting_comments_before_errors(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingCommentsBeforeErrors") {
        return;
    }
    let content = r"namespace A {
    interface B {
        // a
        // b
        baz();
/*0*/        // d /*1*/asd a
        // e
        foo();
        // f asd
        // g as
        bar();
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.go_to_marker(t, "0");
    f.verify_current_line_content(t, "        // d ");
    done();
}
