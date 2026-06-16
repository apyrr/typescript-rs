#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_js_module_exports() {
    let mut t = TestingT;
    run_test_go_to_definition_js_module_exports(&mut t);
}

fn run_test_go_to_definition_js_module_exports(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @Filename: foo.js
x./*def*/test = () => { }
x.[|/*ref*/test|]();
x./*defFn*/test3 = function () { }
x.[|/*refFn*/test3|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["ref".to_string(), "refFn".to_string()]);
    done();
}
