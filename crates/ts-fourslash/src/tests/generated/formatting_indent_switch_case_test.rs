#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_indent_switch_case() {
    let mut t = TestingT;
    run_test_formatting_indent_switch_case(&mut t);
}

fn run_test_formatting_indent_switch_case(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"let foo = 1;
switch (foo) {
/*1*/case 0:
/*2*/break;
/*3*/default:
/*4*/break;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.indent_switch_case = ts_core::TSTrue;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "2");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "3");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "4");
    f.verify_indentation(t, 8);
    {
        let mut opts = f.get_options();
        opts.format_code_settings.indent_switch_case = ts_core::TSFalse;
        f.configure(t, opts);
    }
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_indentation(t, 0);
    f.go_to_marker(t, "2");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "3");
    f.verify_indentation(t, 0);
    f.go_to_marker(t, "4");
    f.verify_indentation(t, 4);
    done();
}
