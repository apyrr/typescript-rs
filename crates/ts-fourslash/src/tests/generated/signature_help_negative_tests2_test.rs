#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_negative_tests2() {
    let mut t = TestingT;
    run_test_signature_help_negative_tests2(&mut t);
}

fn run_test_signature_help_negative_tests2(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpNegativeTests2") {
        return;
    }
    let content = r"class clsOverload { constructor(); constructor(test: string); constructor(test?: string) { } }
var x = new clsOverload/*beforeOpenParen*/()/*afterCloseParen*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(
        t,
        &vec!["beforeOpenParen".to_string(), "afterCloseParen".to_string()],
    );
    done();
}
