#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_if_in_else_block() {
    let mut t = TestingT;
    run_test_formatting_if_in_else_block(&mut t);
}

fn run_test_formatting_if_in_else_block(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"if (true) {
}
else {
    if (true) {
        /*1*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "}");
    f.verify_current_line_content(t, "    }");
    done();
}
