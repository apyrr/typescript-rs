#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_colon_and_q_mark() {
    let mut t = TestingT;
    run_test_format_colon_and_q_mark(&mut t);
}

fn run_test_format_colon_and_q_mark(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatColonAndQMark") {
        return;
    }
    let content = r"class foo {/*1*/
    constructor (n?: number, m = 5, o?: string) { }/*2*/
    x:number = 1?2:3;/*3*/
}/*4*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "class foo {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    constructor(n?: number, m = 5, o?: string) { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    x: number = 1 ? 2 : 3;");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "}");
    done();
}
