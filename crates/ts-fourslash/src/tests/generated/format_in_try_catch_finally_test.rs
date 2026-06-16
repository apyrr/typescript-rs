#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_in_try_catch_finally() {
    let mut t = TestingT;
    run_test_format_in_try_catch_finally(&mut t);
}

fn run_test_format_in_try_catch_finally(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatInTryCatchFinally") {
        return;
    }
    let content = r"try 
{
    var x = 1/*1*/
}
catch (e) 
{
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, ";");
    f.verify_current_line_content(t, "    var x = 1;");
    done();
}
