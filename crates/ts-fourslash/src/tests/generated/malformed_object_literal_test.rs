#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_malformed_object_literal() {
    let mut t = TestingT;
    run_test_malformed_object_literal(&mut t);
}

fn run_test_malformed_object_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var tt = { aa };/**/
var y = /*1*/"unclosed string literal
/*2*/var x = "closed string literal""#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_error_exists_before_marker(&f.marker_by_name(""), 0);
    f.verify_error_exists_after_marker_name("1");
    f.verify_no_error_exists_after_marker_name("2");
    done();
}
