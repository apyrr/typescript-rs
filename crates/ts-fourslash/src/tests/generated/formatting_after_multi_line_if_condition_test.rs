#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_after_multi_line_if_condition() {
    let mut t = TestingT;
    run_test_formatting_after_multi_line_if_condition(&mut t);
}

fn run_test_formatting_after_multi_line_if_condition(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingAfterMultiLineIfCondition") {
        return;
    }
    let content = r" var foo;
 if (foo &&
     foo) {
/*comment*/     // This is a comment
     foo.toString();
 /**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "}");
    f.go_to_marker(t, "comment");
    f.verify_current_line_content(t, "    // This is a comment");
    done();
}
