#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_filtered_triggers03() {
    let mut t = TestingT;
    run_test_signature_help_filtered_triggers03(&mut t);
}

fn run_test_signature_help_filtered_triggers03(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpFilteredTriggers03") {
        return;
    }
    let content = r"declare class ViewJayEss {
    constructor(obj: object);
}
new ViewJayEss({
    methods: {
        sayHello/**/
    }
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "(");
    f.verify_no_signature_help_with_context(
        t,
        Some(SignatureHelpContext {
            is_retrigger: false,
            trigger_character: Some("(".to_string()),
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::TRIGGER_CHARACTER),
        }),
    );
    f.insert(t, ") {},");
    f.verify_no_signature_help_with_context(
        t,
        Some(SignatureHelpContext {
            is_retrigger: false,
            trigger_character: Some(",".to_string()),
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::TRIGGER_CHARACTER),
        }),
    );
    done();
}
