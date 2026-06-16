#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_module_dot_exports() {
    let mut t = TestingT;
    run_test_find_all_refs_module_dot_exports(&mut t);
}

fn run_test_find_all_refs_module_dot_exports(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsModuleDotExports") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: /a.js
/*1*/const b = require("/*2*/./b");
// @Filename: /b.js
/*3*/module.exports = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
