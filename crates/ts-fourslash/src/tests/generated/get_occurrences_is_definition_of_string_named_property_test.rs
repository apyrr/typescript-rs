#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_is_definition_of_string_named_property() {
    let mut t = TestingT;
    run_test_get_occurrences_is_definition_of_string_named_property(&mut t);
}

fn run_test_get_occurrences_is_definition_of_string_named_property(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOccurrencesIsDefinitionOfStringNamedProperty") {
        return;
    }
    let content = r#"let o = { /*1*/"/*2*/x": 12 };
let y = o./*3*/x;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
