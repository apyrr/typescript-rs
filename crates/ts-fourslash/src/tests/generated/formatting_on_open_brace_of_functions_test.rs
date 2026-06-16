#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_open_brace_of_functions() {
    let mut t = TestingT;
    run_test_formatting_on_open_brace_of_functions(&mut t);
}

fn run_test_formatting_on_open_brace_of_functions(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/**/function T2_y()
{
Plugin.T1.t1_x();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "");
    f.verify_current_line_content(t, "function T2_y() {");
    done();
}
