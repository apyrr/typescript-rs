#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_catch_clause() {
    let mut t = TestingT;
    run_test_find_all_refs_catch_clause(&mut t);
}

fn run_test_find_all_refs_catch_clause(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"try { }
catch (/*1*/err) {
    /*2*/err;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
