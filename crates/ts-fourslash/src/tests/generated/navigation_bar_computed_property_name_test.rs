#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_computed_property_name() {
    let mut t = TestingT;
    run_test_navigation_bar_computed_property_name(&mut t);
}

fn run_test_navigation_bar_computed_property_name(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarComputedPropertyName") {
        return;
    }
    let content = r#"function F(key, value) {
    return {
        [key]: value,
        "prop": true
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
