#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_range_ending_after_comma_of_call() {
    let mut t = TestingT;
    run_test_format_range_ending_after_comma_of_call(&mut t);
}

fn run_test_format_range_ending_after_comma_of_call(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"someCall(
    /*start*/"firstParameter",/*end*/
    "something else"
);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "start", "end");
    done();
}
