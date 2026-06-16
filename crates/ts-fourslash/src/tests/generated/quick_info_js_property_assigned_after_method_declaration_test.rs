#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_property_assigned_after_method_declaration() {
    let mut t = TestingT;
    run_test_quick_info_js_property_assigned_after_method_declaration(&mut t);
}

fn run_test_quick_info_js_property_assigned_after_method_declaration(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsPropertyAssignedAfterMethodDeclaration") {
        return;
    }
    let content = r"// @noLib: true
// @allowJs: true
// @noImplicitThis: true
// @Filename: /a.js
const o = {
    test/*1*/() {
        this./*2*/test = 0;
    }
};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(method) test(): void", "");
    f.verify_quick_info_at(t, "2", "(method) test(): void", "");
    done();
}
