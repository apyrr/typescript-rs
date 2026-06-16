#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_on_declaration() {
    let mut t = TestingT;
    run_test_signature_help_on_declaration(&mut t);
}

fn run_test_signature_help_on_declaration(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpOnDeclaration") {
        return;
    }
    let content = r"function f</**/
x";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers(t, &vec!["".to_string()]);
    done();
}
