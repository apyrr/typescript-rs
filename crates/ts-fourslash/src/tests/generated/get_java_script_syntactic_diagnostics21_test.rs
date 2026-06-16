#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_java_script_syntactic_diagnostics21() {
    let mut t = TestingT;
    run_test_get_java_script_syntactic_diagnostics21(&mut t);
}

fn run_test_get_java_script_syntactic_diagnostics21(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @experimentalDecorators: true
// @Filename: a.js
@internal class C {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_non_suggestion_diagnostics(&[]);
    done();
}
