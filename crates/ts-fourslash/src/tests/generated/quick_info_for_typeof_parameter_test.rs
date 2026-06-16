#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_typeof_parameter() {
    let mut t = TestingT;
    run_test_quick_info_for_typeof_parameter(&mut t);
}

fn run_test_quick_info_for_typeof_parameter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo() {
    var y/*ref1*/1: string;
    var x: typeof y/*ref2*/1;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "ref1", "(local var) y1: string", "");
    f.verify_quick_info_at(t, "ref2", "(local var) y1: string", "");
    done();
}
