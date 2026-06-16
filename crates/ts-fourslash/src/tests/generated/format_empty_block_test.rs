#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_empty_block() {
    let mut t = TestingT;
    run_test_format_empty_block(&mut t);
}

fn run_test_format_empty_block(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"{}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_eof(t);
    f.insert(t, "\n");
    f.go_to_bof(t);
    f.verify_current_line_content(t, "{ }");
    done();
}
