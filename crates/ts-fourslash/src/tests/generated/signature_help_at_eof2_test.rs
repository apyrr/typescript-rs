#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_at_eof2() {
    let mut t = TestingT;
    run_test_signature_help_at_eof2(&mut t);
}

fn run_test_signature_help_at_eof2(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpAtEOF2") {
        return;
    }
    let content = r"console.log()
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_signature_help_for_markers_with_context(
        t,
        &vec!["".to_string()],
        Some(SignatureHelpContext {
            is_retrigger: false,
            trigger_character: None,
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::INVOKED),
        }),
    );
    done();
}
