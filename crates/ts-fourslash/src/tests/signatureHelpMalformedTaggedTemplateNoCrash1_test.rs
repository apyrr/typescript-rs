use crate::{new_fourslash, SignatureHelpContext, TestingT};
use ts_lsproto as lsproto;

pub fn test_signature_help_malformed_tagged_template_no_crash1(t: &mut TestingT) {
    let content = "`${1}\n/*m1*/\n// ``\n";

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.go_to_marker(t, "m1");
    f.verify_no_signature_help_with_context(
        t,
        Some(SignatureHelpContext {
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::Invoked),
            trigger_character: None,
            is_retrigger: false,
        }),
    );
    done();
}

