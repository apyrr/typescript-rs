#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tabbing_after_newline_inserted_before_while() {
    let mut t = TestingT;
    run_test_tabbing_after_newline_inserted_before_while(&mut t);
}

fn run_test_tabbing_after_newline_inserted_before_while(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo() {
    /**/while (true) { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert_line(t, "");
    f.verify_current_line_content(t, "    while (true) { }");
    done();
}
