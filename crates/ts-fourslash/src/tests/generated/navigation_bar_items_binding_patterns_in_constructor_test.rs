#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_binding_patterns_in_constructor() {
    let mut t = TestingT;
    run_test_navigation_bar_items_binding_patterns_in_constructor(&mut t);
}

fn run_test_navigation_bar_items_binding_patterns_in_constructor(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsBindingPatternsInConstructor") {
        return;
    }
    let content = r"class A {
    x: any
    constructor([a]: any) {
    }
}
class B {
    x: any;
    constructor( {a} = { a: 1 }) {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
