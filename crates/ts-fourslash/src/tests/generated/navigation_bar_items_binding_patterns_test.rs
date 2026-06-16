#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_binding_patterns() {
    let mut t = TestingT;
    run_test_navigation_bar_items_binding_patterns(&mut t);
}

fn run_test_navigation_bar_items_binding_patterns(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsBindingPatterns") {
        return;
    }
    let content = r"'use strict'
var foo, {}
var bar, []
let foo1, {a, b}
const bar1, [c, d]
var {e, x: [f, g]} = {a:1, x:[]};
var { h: i = function j() {} } = obj;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
