#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_void() {
    let mut t = TestingT;
    run_test_formatting_void(&mut t);
}

fn run_test_formatting_void(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/  var x: () =>           void    ;
/*2*/  var y:     void    ;
/*3*/  function test(a:void,b:string){}
/*4*/  var a, b, c, d;
/*5*/  void    a    ;
/*6*/  void        (0);
/*7*/  b=void(c=1,d=2);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "var x: () => void;");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "var y: void;");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "function test(a: void, b: string) { }");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "void a;");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "void (0);");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "b = void (c = 1, d = 2);");
    done();
}
