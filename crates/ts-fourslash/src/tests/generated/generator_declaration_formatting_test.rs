#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generator_declaration_formatting() {
    let mut t = TestingT;
    run_test_generator_declaration_formatting(&mut t);
}

fn run_test_generator_declaration_formatting(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function    *g() { }/*1*/
var v = function    *() { };/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "function* g() { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "var v = function*() { };");
    done();
}
