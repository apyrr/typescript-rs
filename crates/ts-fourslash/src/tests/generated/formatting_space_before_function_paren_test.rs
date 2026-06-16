#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_space_before_function_paren() {
    let mut t = TestingT;
    run_test_formatting_space_before_function_paren(&mut t);
}

fn run_test_formatting_space_before_function_paren(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/function foo() { }
/*2*/function boo  () { }
/*3*/var bar = function foo() { };
/*4*/var foo = { bar() { } };
/*5*/function tmpl <T> () { }
/*6*/var f = function*() { };
/*7*/function* g () { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_before_function_parenthesis = ts_core::TSTrue;
        f.configure(t, opts);
    }
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_function_keyword_for_anonymous_functions = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "function foo () { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "function boo () { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "var bar = function foo () { };");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "var foo = { bar () { } };");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "function tmpl<T> () { }");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "var f = function*() { };");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "function* g () { }");
    done();
}
