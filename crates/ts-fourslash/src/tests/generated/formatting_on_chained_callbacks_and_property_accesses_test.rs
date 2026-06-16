#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_chained_callbacks_and_property_accesses() {
    let mut t = TestingT;
    run_test_formatting_on_chained_callbacks_and_property_accesses(&mut t);
}

fn run_test_formatting_on_chained_callbacks_and_property_accesses(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var x = 1;
x
/*1*/.toFixed
x
/*2*/.toFixed()
x
/*3*/.toFixed()
/*4*/.length
/*5*/.toString();
x
/*6*/.toFixed
/*7*/.toString()
/*8*/.length;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    .toFixed");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    .toFixed()");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    .toFixed()");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    .length");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    .toString();");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    .toFixed");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "    .toString()");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    .length;");
    done();
}
