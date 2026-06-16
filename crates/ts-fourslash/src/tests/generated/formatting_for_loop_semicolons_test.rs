#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_for_loop_semicolons() {
    let mut t = TestingT;
    run_test_formatting_for_loop_semicolons(&mut t);
}

fn run_test_formatting_for_loop_semicolons(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/for (;;) { }
/*2*/for (var x;x<0;x++) { }
/*3*/for (var x ;x<0 ;x++) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "for (; ;) { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "for (var x; x < 0; x++) { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "for (var x; x < 0; x++) { }");
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_semicolon_in_for_statements = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "for (;;) { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "for (var x;x < 0;x++) { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "for (var x;x < 0;x++) { }");
    done();
}
