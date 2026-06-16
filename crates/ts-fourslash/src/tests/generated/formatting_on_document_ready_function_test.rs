#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_document_ready_function() {
    let mut t = TestingT;
    run_test_formatting_on_document_ready_function(&mut t);
}

fn run_test_formatting_on_document_ready_function(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/$    (   document   )   .  ready  (   function   (   )   {
/*2*/    alert    (           'i am ready'  )   ;
/*3*/           }                 );";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "$(document).ready(function() {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    alert('i am ready');");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "});");
    done();
}
