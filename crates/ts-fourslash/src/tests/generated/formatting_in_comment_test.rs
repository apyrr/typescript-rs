#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_in_comment() {
    let mut t = TestingT;
    run_test_formatting_in_comment(&mut t);
}

fn run_test_formatting_in_comment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class A {
foo(              ); // /*1*/
}
function foo() {       var x;       } // /*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, ";");
    f.verify_current_line_content(t, "foo(              ); // ;");
    f.go_to_marker(t, "2");
    f.insert(t, "}");
    f.verify_current_line_content(t, "function foo() {       var x;       } // }");
    done();
}
