#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_force_indent_after_new_line_insert() {
    let mut t = TestingT;
    run_test_force_indent_after_new_line_insert(&mut t);
}

fn run_test_force_indent_after_new_line_insert(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function f1()
{ return 0; }
function f2()
{
return 0;
}
function g()
{ function h() {
return 0;
}}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"function f1() { return 0; }
function f2() {
    return 0;
}
function g() {
    function h() {
        return 0;
    }
}",
    );
    done();
}
