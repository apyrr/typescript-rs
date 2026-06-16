#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_js_missing_identifier() {
    let mut t = TestingT;
    run_test_signature_help_js_missing_identifier(&mut t);
}

fn run_test_signature_help_js_missing_identifier(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @checkJs: true
// @Filename: test.js
log(/**/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &vec!["".to_string()]);
    done();
}
