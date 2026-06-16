#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_semicolon_formatting() {
    let mut t = TestingT;
    run_test_semicolon_formatting(&mut t);
}

fn run_test_semicolon_formatting(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/**/function of1 (b:{r:{c:number";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_eof(t);
    f.insert(t, ";");
    f.verify_current_line_content(t, "function of1(b: { r: { c: number;");
    done();
}
