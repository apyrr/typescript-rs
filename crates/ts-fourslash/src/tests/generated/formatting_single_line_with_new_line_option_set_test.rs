#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_single_line_with_new_line_option_set() {
    let mut t = TestingT;
    run_test_formatting_single_line_with_new_line_option_set(&mut t);
}

fn run_test_formatting_single_line_with_new_line_option_set(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingSingleLineWithNewLineOptionSet") {
        return;
    }
    let content = r"/*1*/namespace Default{}
/*2*/function foo(){}
/*3*/if (true){}
/*4*/function boo() {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_functions = ts_core::TSTrue;
        f.configure(t, opts);
    }
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .place_open_brace_on_new_line_for_control_blocks = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "namespace Default { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "function foo() { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "if (true) { }");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "function boo()");
    done();
}
