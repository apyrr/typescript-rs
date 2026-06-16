#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_class4() {
    let mut t = TestingT;
    run_test_navigation_bar_items_class4(&mut t);
}

fn run_test_navigation_bar_items_class4(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsClass4") {
        return;
    }
    let content = r"// @allowJs: true
// @filename: /foo.js
class Foo {}
function Foo() {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
