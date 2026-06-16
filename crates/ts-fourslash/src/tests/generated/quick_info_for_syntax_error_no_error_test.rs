#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_syntax_error_no_error() {
    let mut t = TestingT;
    run_test_quick_info_for_syntax_error_no_error(&mut t);
}

fn run_test_quick_info_for_syntax_error_no_error(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace X {
    export =
}
X.add/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "any", "");
    done();
}
