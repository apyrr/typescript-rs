#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_on_accessors01() {
    let mut t = TestingT;
    run_test_smart_indent_on_accessors01(&mut t);
}

fn run_test_smart_indent_on_accessors01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Foo {
    get foo(a,
            /*1*/b,/*0*/
                 //comment/*2*/
        /*3*/c
        ) {
    }
    set foo(a,
            /*5*/b,/*4*/
                 //comment/*6*/
        /*7*/c
        ) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "0");
    f.insert(t, "\n");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "        b,");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "                 //comment");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "        c");
    f.go_to_marker(t, "4");
    f.insert(t, "\n");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "        b,");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "                 //comment");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        c");
    done();
}
