#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unclosed_string_literal_error_recovery2() {
    let mut t = TestingT;
    run_test_unclosed_string_literal_error_recovery2(&mut t);
}

fn run_test_unclosed_string_literal_error_recovery2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function alpha() {

    var x = "x\
    var y = 1;
    function beta() { }
    beta(;
    /**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_error_exists_after_marker_name("");
    done();
}
