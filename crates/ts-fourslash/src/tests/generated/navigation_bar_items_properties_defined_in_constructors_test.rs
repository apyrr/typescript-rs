#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_properties_defined_in_constructors() {
    let mut t = TestingT;
    run_test_navigation_bar_items_properties_defined_in_constructors(&mut t);
}

fn run_test_navigation_bar_items_properties_defined_in_constructors(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class List<T> {
    constructor(public a: boolean, private b: T, readonly c: string, d: number) {
        var local = 0;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
