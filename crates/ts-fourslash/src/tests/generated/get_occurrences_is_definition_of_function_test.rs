#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_is_definition_of_function() {
    let mut t = TestingT;
    run_test_get_occurrences_is_definition_of_function(&mut t);
}

fn run_test_get_occurrences_is_definition_of_function(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesIsDefinitionOfFunction") {
        return;
    }
    let content = r"/*1*/function /*2*/func(x: number) {
}
/*3*/func(x)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
