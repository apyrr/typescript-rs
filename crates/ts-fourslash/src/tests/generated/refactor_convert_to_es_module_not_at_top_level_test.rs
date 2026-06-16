#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_refactor_convert_to_es_module_not_at_top_level() {
    let mut t = TestingT;
    run_test_refactor_convert_to_es_module_not_at_top_level(&mut t);
}

fn run_test_refactor_convert_to_es_module_not_at_top_level(t: &mut TestingT) {
    if should_skip_if_failing("TestRefactorConvertToEsModule_notAtTopLevel") {
        return;
    }
    let content = r"// @allowJs: true
// @target: esnext
// @Filename: /a.js
(function() {
    module.exports = 0;
})();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
