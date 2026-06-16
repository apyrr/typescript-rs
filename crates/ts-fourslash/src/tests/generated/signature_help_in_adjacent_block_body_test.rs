#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_in_adjacent_block_body() {
    let mut t = TestingT;
    run_test_signature_help_in_adjacent_block_body(&mut t);
}

fn run_test_signature_help_in_adjacent_block_body(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpInAdjacentBlockBody") {
        return;
    }
    let content = r"declare function foo(...args);

foo(() => {/*1*/}/*2*/)";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_present_with_context(
        t,
        Some(SignatureHelpContext {
            is_retrigger: false,
            trigger_character: None,
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::INVOKED),
        }),
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_present_with_context(
        t,
        Some(SignatureHelpContext {
            is_retrigger: false,
            trigger_character: None,
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::INVOKED),
        }),
    );
    done();
}
