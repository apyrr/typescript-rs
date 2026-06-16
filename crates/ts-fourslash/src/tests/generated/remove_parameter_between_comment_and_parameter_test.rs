#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_parameter_between_comment_and_parameter() {
    let mut t = TestingT;
    run_test_remove_parameter_between_comment_and_parameter(&mut t);
}

fn run_test_remove_parameter_between_comment_and_parameter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function fn(/* comment! */ /**/a: number, c) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.delete_at_caret(t, 10);
    done();
}
