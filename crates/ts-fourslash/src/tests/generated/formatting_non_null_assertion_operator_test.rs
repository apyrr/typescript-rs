#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_non_null_assertion_operator() {
    let mut t = TestingT;
    run_test_formatting_non_null_assertion_operator(&mut t);
}

fn run_test_formatting_non_null_assertion_operator(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/ 'bar' ! ;
/*2*/ ( 'bar' ) ! ;
/*3*/ 'bar' [ 1 ] ! ;
/*4*/ var  bar  =  'bar' . foo ! ;
/*5*/ var  foo  =  bar ! ;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "'bar'!;");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "('bar')!;");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "'bar'[1]!;");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "var bar = 'bar'.foo!;");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "var foo = bar!;");
    done();
}
