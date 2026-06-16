#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_jsx() {
    let mut t = TestingT;
    run_test_signature_help_jsx(&mut t);
}

fn run_test_signature_help_jsx(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpJSX") {
        return;
    }
    let content = r"//@Filename: test.tsx
//@jsx: react
declare var React: any;
const z = <div>{[].map(x => </**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_no_signature_help_with_context(
        t,
        Some(SignatureHelpContext {
            is_retrigger: false,
            trigger_character: Some("<".to_string()),
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::TRIGGER_CHARACTER),
        }),
    );
    done();
}
