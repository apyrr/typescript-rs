#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_string_literal_types() {
    let mut t = TestingT;
    run_test_find_all_refs_for_string_literal_types(&mut t);
}

fn run_test_find_all_refs_for_string_literal_types(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"type Options = "/*1*/option 1" | "option 2";
let myOption: Options = "/*2*/option 1";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
