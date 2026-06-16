#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_space_before_close_paren() {
    let mut t = TestingT;
    run_test_formatting_space_before_close_paren(&mut t);
}

fn run_test_formatting_space_before_close_paren(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/({});
/*2*/(  {});
/*3*/({foo:42});
/*4*/(  {foo:42}  );
/*5*/var bar = (function (a) { });";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_opening_and_before_closing_nonempty_parenthesis = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "( {} );");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "( {} );");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "( { foo: 42 } );");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "( { foo: 42 } );");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "var bar = ( function( a ) { } );");
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_opening_and_before_closing_nonempty_parenthesis = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "({});");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "({});");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "({ foo: 42 });");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "({ foo: 42 });");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "var bar = (function(a) { });");
    done();
}
