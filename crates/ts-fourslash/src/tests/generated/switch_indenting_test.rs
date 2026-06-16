#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_switch_indenting() {
    let mut t = TestingT;
    run_test_switch_indenting(&mut t);
}

fn run_test_switch_indenting(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"switch (null) {
    case 0:
        /**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "case 1:\n");
    f.verify_indentation(t, 8);
    done();
}
