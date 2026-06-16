#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_is_definition_shorthand_property() {
    let mut t = TestingT;
    run_test_is_definition_shorthand_property(&mut t);
}

fn run_test_is_definition_shorthand_property(t: &mut TestingT) {
    if should_skip_if_failing("TestIsDefinitionShorthandProperty") {
        return;
    }
    let content = r"const /*1*/x = 1;
const y: { /*2*/x: number } = { /*3*/x };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
