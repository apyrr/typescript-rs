#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_nested_statements() {
    let mut t = TestingT;
    run_test_formatting_on_nested_statements(&mut t);
}

fn run_test_formatting_on_nested_statements(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"{
/*1*/{
/*3*/test
}/*2*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "1", "2");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    {");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "        test");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    }");
    done();
}
