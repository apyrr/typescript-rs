use crate::{new_fourslash, SignatureHelpContext, TestingT};
use ts_lsproto as lsproto;

pub fn test_signature_help_nested_call_trailing_comma(t: &mut TestingT) {
    // Regression test for crash when requesting signature help on a call target
    // where the nested call has a trailing comma.
    // Both outer and inner calls must have trailing commas, and outer must be generic.
    let content = r#"declare function outer<T>(range: T): T;
declare function inner(a: any): any;

outer(inner/*1*/(undefined,),);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_present_with_context(
        t,
        Some(SignatureHelpContext {
            is_retrigger: false,
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::Invoked),
            trigger_character: None,
        }),
    );
    done();
}

