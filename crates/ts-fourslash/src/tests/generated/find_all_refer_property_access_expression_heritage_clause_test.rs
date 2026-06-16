#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refer_property_access_expression_heritage_clause() {
    let mut t = TestingT;
    run_test_find_all_refer_property_access_expression_heritage_clause(&mut t);
}

fn run_test_find_all_refer_property_access_expression_heritage_clause(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class B {}
function foo() {
    return {/*1*/B: B};
}
class C extends (foo())./*2*/B {}
class C1 extends foo()./*3*/B {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
