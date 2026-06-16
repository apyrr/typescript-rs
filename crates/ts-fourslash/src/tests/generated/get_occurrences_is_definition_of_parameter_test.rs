#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_is_definition_of_parameter() {
    let mut t = TestingT;
    run_test_get_occurrences_is_definition_of_parameter(&mut t);
}

fn run_test_get_occurrences_is_definition_of_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesIsDefinitionOfParameter") {
        return;
    }
    let content = r"function f(/*1*/x: number) {
  return /*2*/x + 1
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
