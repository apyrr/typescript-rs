#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_illegal_assignment() {
    let mut t = TestingT;
    run_test_references_for_illegal_assignment(&mut t);
}

fn run_test_references_for_illegal_assignment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"f/*1*/oo = fo/*2*/o;
var /*bar*/bar = function () { };
bar = bar + 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &["1".to_string(), "2".to_string(), "bar".to_string()],
    );
    done();
}
