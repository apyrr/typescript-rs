#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_on_enter_function_declaration() {
    let mut t = TestingT;
    run_test_format_on_enter_function_declaration(&mut t);
}

fn run_test_format_on_enter_function_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*0*/function listAPIFiles(path: string): string[] {/*1*/ }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.go_to_marker(t, "0");
    f.verify_current_line_content(t, "function listAPIFiles(path: string): string[] {");
    done();
}
