#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_syntactic_diagnostics16() {
    let mut t = TestingT;
    run_test_get_java_script_syntactic_diagnostics16(&mut t);
}

fn run_test_get_java_script_syntactic_diagnostics16(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJavaScriptSyntacticDiagnostics16") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: a.js
function F(p?) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_non_suggestion_diagnostics(t);
    done();
}
