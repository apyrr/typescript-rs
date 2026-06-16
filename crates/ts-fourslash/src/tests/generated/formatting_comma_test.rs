#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_comma() {
    let mut t = TestingT;
    run_test_formatting_comma(&mut t);
}

fn run_test_formatting_comma(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var x = [1 , 2];/*x*/
var y = ( 1  , 2 );/*y*/
var z1 = 1 , zz = 2;/*z1*/
var z2 = {
    x: 1 ,/*z2*/
    y: 2
};
var z3 = (
    () => { }  ,/*z3*/
    () => { }
    );
var z4 = [
    () => { } ,/*z4*/
    () => { }
];
var z5 = {
    x: () => { } ,/*z5*/
    y: () => { }
}; ";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "x");
    f.verify_current_line_content(t, "var x = [1, 2];");
    f.go_to_marker(t, "y");
    f.verify_current_line_content(t, "var y = (1, 2);");
    f.go_to_marker(t, "z1");
    f.verify_current_line_content(t, "var z1 = 1, zz = 2;");
    f.go_to_marker(t, "z2");
    f.verify_current_line_content(t, "    x: 1,");
    f.go_to_marker(t, "z3");
    f.verify_current_line_content(t, "    () => { },");
    f.go_to_marker(t, "z4");
    f.verify_current_line_content(t, "    () => { },");
    f.go_to_marker(t, "z5");
    f.verify_current_line_content(t, "    x: () => { },");
    done();
}
