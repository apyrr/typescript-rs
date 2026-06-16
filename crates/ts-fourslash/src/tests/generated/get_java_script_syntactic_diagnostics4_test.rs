#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_syntactic_diagnostics4() {
    let mut t = TestingT;
    run_test_get_java_script_syntactic_diagnostics4(&mut t);
}

fn run_test_get_java_script_syntactic_diagnostics4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @Filename: a.js
public class C { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_non_suggestion_diagnostics(t);
    done();
}
