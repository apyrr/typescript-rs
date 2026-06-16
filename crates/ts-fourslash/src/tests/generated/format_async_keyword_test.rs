#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_async_keyword() {
    let mut t = TestingT;
    run_test_format_async_keyword(&mut t);
}

fn run_test_format_async_keyword(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatAsyncKeyword") {
        return;
    }
    let content = r"/*1*/let x = async         () => 1;
/*2*/let y = async() => 1;
/*3*/let z = async    function   () { return 1; };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "let x = async () => 1;");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "let y = async () => 1;");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "let z = async function() { return 1; };");
    done();
}
