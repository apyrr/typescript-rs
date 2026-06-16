#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_navigation_bar_items() {
    let mut t = TestingT;
    run_test_get_navigation_bar_items(&mut t);
}

fn run_test_get_navigation_bar_items(t: &mut TestingT) {
    if should_skip_if_failing("TestGetNavigationBarItems") {
        return;
    }
    let content = r#"class C {
    foo;
    ["bar"]: string;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
