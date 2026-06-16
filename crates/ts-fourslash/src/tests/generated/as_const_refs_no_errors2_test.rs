#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_as_const_refs_no_errors2() {
    let mut t = TestingT;
    run_test_as_const_refs_no_errors2(&mut t);
}

fn run_test_as_const_refs_no_errors2(t: &mut TestingT) {
    if should_skip_if_failing("TestAsConstRefsNoErrors2") {
        return;
    }
    let content = r"class Tex {
    type = </**/const>'Text';
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["".to_string()]);
    f.verify_no_errors();
    done();
}
