#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_enter_in_comments() {
    let mut t = TestingT;
    run_test_formatting_on_enter_in_comments(&mut t);
}

fn run_test_formatting_on_enter_in_comments(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace me {
    class A {
        /*
         */*1*/
    /*2*/}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert_line(t, "");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    }");
    done();
}
