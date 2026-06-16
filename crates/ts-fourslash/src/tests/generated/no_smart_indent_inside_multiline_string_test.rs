#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_no_smart_indent_inside_multiline_string() {
    let mut t = TestingT;
    run_test_no_smart_indent_inside_multiline_string(&mut t);
}

fn run_test_no_smart_indent_inside_multiline_string(t: &mut TestingT) {
    if should_skip_if_failing("TestNoSmartIndentInsideMultilineString") {
        return;
    }
    let content = r"window.onload = () => {
    var el = document.getElementById('content\/*1*/');
    var greeter = new Greeter(el);
greeter.start();
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 0);
    done();
}
