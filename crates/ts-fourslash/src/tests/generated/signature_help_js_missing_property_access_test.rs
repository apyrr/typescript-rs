#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_js_missing_property_access() {
    let mut t = TestingT;
    run_test_signature_help_js_missing_property_access(&mut t);
}

fn run_test_signature_help_js_missing_property_access(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpJSMissingPropertyAccess") {
        return;
    }
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: test.js
foo.filter(/**/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
