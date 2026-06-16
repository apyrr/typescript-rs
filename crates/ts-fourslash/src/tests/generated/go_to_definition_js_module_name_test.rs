#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_js_module_name() {
    let mut t = TestingT;
    run_test_go_to_definition_js_module_name(&mut t);
}

fn run_test_go_to_definition_js_module_name(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionJsModuleName") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: foo.js
/*2*/module.exports = {};
// @Filename: bar.js
var x = require([|/*1*/"./foo"|]);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}
