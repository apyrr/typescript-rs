#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_is_definition_of_namespace() {
    let mut t = TestingT;
    run_test_get_occurrences_is_definition_of_namespace(&mut t);
}

fn run_test_get_occurrences_is_definition_of_namespace(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesIsDefinitionOfNamespace") {
        return;
    }
    let content = r"/*1*/namespace /*2*/Numbers {
    export var n = 12;
}
let x = /*3*/Numbers.n + 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
