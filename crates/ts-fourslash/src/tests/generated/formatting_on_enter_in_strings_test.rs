#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_enter_in_strings() {
    let mut t = TestingT;
    run_test_formatting_on_enter_in_strings(&mut t);
}

fn run_test_formatting_on_enter_in_strings(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var x = /*1*/"unclosed string literal\/*2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "2");
    f.insert_line(t, "");
    f.insert_line(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "var x = \"unclosed string literal\\");
    done();
}
