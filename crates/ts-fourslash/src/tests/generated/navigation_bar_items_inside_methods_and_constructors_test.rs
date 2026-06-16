#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_inside_methods_and_constructors() {
    let mut t = TestingT;
    run_test_navigation_bar_items_inside_methods_and_constructors(&mut t);
}

fn run_test_navigation_bar_items_inside_methods_and_constructors(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsInsideMethodsAndConstructors") {
        return;
    }
    let content = r"class Class {
    constructor() {
        function LocalFunctionInConstructor() {}
        interface LocalInterfaceInConstrcutor {}
        enum LocalEnumInConstructor { LocalEnumMemberInConstructor }
    }

    method() {
        function LocalFunctionInMethod() {
            function LocalFunctionInLocalFunctionInMethod() {}
        }
        interface LocalInterfaceInMethod {}
        enum LocalEnumInMethod { LocalEnumMemberInMethod }
    }

    emptyMethod() { } // Non child functions method should not be duplicated
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
