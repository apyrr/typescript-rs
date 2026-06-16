#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_implicit_module() {
    let mut t = TestingT;
    run_test_format_implicit_module(&mut t);
}

fn run_test_format_implicit_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"       export class A {

       }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_bof(t);
    f.verify_current_line_content(t, "export class A {");
    f.go_to_eof(t);
    f.verify_current_line_content(t, "}");
    done();
}
