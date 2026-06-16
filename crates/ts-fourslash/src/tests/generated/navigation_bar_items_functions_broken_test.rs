#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_functions_broken() {
    let mut t = TestingT;
    run_test_navigation_bar_items_functions_broken(&mut t);
}

fn run_test_navigation_bar_items_functions_broken(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsFunctionsBroken") {
        return;
    }
    let content = r"function f() {
    function;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
