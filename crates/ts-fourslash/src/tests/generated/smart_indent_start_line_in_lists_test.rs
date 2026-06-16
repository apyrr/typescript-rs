#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_start_line_in_lists() {
    let mut t = TestingT;
    run_test_smart_indent_start_line_in_lists(&mut t);
}

fn run_test_smart_indent_start_line_in_lists(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"foo(function () {
}).then(function () {/*1*/
})";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    done();
}
