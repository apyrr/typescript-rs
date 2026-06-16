#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_conditional_operator() {
    let mut t = TestingT;
    run_test_formatting_conditional_operator(&mut t);
}

fn run_test_formatting_conditional_operator(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var x=true?1:2";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_bof(t);
    f.verify_current_line_content(t, "var x = true ? 1 : 2");
    done();
}
