#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_empty_param_list() {
    let mut t = TestingT;
    run_test_format_empty_param_list(&mut t);
}

fn run_test_format_empty_param_list(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatEmptyParamList") {
        return;
    }
    let content = r"function f( f: function){/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "}");
    f.verify_current_line_content(t, "function f(f: function) { }");
    done();
}
