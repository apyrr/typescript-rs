#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_space_after_return() {
    let mut t = TestingT;
    run_test_space_after_return(&mut t);
}

fn run_test_space_after_return(t: &mut TestingT) {
    if should_skip_if_failing("TestSpaceAfterReturn") {
        return;
    }
    let content = r"function f( ) {
return       1;/*1*/
return[1];/*2*/
return    ;/*3*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    return 1;");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    return [1];");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    return;");
    done();
}
