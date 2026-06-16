#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_in_class_expression() {
    let mut t = TestingT;
    run_test_find_all_refs_in_class_expression(&mut t);
}

fn run_test_find_all_refs_in_class_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I { /*0*/boom(): void; }
new class C implements I {
   /*1*/boom(){}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string()]);
    done();
}
