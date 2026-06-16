#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_space_between_parent() {
    let mut t = TestingT;
    run_test_formatting_space_between_parent(&mut t);
}

fn run_test_formatting_space_between_parent(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingSpaceBetweenParent") {
        return;
    }
    let content = r"/*1*/foo(() => 1);
/*2*/foo(1);
/*3*/if((true)){}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings
            .insert_space_after_opening_and_before_closing_nonempty_parenthesis = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "foo( () => 1 );");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "foo( 1 );");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "if ( ( true ) ) { }");
    done();
}
