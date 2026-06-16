#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_class5() {
    let mut t = TestingT;
    run_test_navigation_bar_items_class5(&mut t);
}

fn run_test_navigation_bar_items_class5(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsClass5") {
        return;
    }
    let content = r"class Foo {}
let Foo = 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
