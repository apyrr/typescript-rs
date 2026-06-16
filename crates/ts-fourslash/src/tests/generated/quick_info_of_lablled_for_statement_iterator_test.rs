#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_of_lablled_for_statement_iterator() {
    let mut t = TestingT;
    run_test_quick_info_of_lablled_for_statement_iterator(&mut t);
}

fn run_test_quick_info_of_lablled_for_statement_iterator(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"label1: for(var /**/i = 0; i < 1; i++) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    done();
}
