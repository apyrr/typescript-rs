#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_syntactic_diagnostics02() {
    let mut t = TestingT;
    run_test_get_java_script_syntactic_diagnostics02(&mut t);
}

fn run_test_get_java_script_syntactic_diagnostics02(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJavaScriptSyntacticDiagnostics02") {
        return;
    }
    let content = r#"// @lib: es5
// @allowJs: true
// @Filename: b.js
var a = "a";
var b: boolean = true;
function foo(): string { }
var var = "c";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_non_suggestion_diagnostics(t);
    done();
}
