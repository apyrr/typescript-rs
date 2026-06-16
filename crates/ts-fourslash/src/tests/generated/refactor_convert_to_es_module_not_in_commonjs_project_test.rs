#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_refactor_convert_to_es_module_not_in_commonjs_project() {
    let mut t = TestingT;
    run_test_refactor_convert_to_es_module_not_in_commonjs_project(&mut t);
}

fn run_test_refactor_convert_to_es_module_not_in_commonjs_project(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @target: es5
// @Filename: /a.js
exports.x = 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
