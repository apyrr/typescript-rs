#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_in_destructuring4() {
    let mut t = TestingT;
    run_test_formatting_in_destructuring4(&mut t);
}

fn run_test_formatting_in_destructuring4(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingInDestructuring4") {
        return;
    }
    let content = r"/*1*/const { 
/*2*/    a,
/*3*/    b,
/*4*/} = { a: 1, b: 2 };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_opening_and_before_closing_nonempty_braces = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "const {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    a,");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    b,");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "} = {a: 1, b: 2};");
    done();
}
