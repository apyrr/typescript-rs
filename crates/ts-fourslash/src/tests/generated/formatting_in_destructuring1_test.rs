#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_in_destructuring1() {
    let mut t = TestingT;
    run_test_formatting_in_destructuring1(&mut t);
}

fn run_test_formatting_in_destructuring1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface let { }
/*1*/var x: let         [];

function foo() {
    'use strict'
/*2*/    let        [x] = [];
/*3*/    const      [x] = [];
/*4*/    for (let[x] = [];x < 1;) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "var x: let[];");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    let [x] = [];");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    const [x] = [];");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    for (let [x] = []; x < 1;) {");
    done();
}
