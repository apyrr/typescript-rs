#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formattingof_single_line_block_constructs() {
    let mut t = TestingT;
    run_test_formattingof_single_line_block_constructs(&mut t);
}

fn run_test_formattingof_single_line_block_constructs(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace InternalModule/*1*/{}
interface MyInterface/*2*/{}
enum E/*3*/{}
class MyClass/*4*/{
constructor()/*cons*/{}
        public MyFunction()/*5*/{return 0;}
public get Getter()/*6*/{}
public set Setter(x)/*7*/{}}
function foo()/*8*/{{}}
(function()/*10*/{});
(() =>/*11*/{});
var x :/*12*/{};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "namespace InternalModule { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "interface MyInterface { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "enum E { }");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "class MyClass {");
    f.go_to_marker(t, "cons");
    f.verify_current_line_content(t, "    constructor() { }");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    public MyFunction() { return 0; }");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    public get Getter() { }");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "    public set Setter(x) { }");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "function foo() { { } }");
    f.go_to_marker(t, "10");
    f.verify_current_line_content(t, "(function() { });");
    f.go_to_marker(t, "11");
    f.verify_current_line_content(t, "(() => { });");
    f.go_to_marker(t, "12");
    f.verify_current_line_content(t, "var x: {};");
    done();
}
