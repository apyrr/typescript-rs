use crate::{new_fourslash, MarkerInput, SignatureHelpCase, SignatureHelpContext, TestingT};
use ts_lsproto as lsproto;

pub fn test_signature_help_token_crash2(t: &mut TestingT) {
    let content = r#"
function foo<T, U>(x: string, y: T, z: U) {

}

foo<number,number>/*1*/("hello", 123,456)
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_signature_help_with_cases(
        t,
        &[SignatureHelpCase {
            marker_input: MarkerInput::Name("1".to_string()),
            expected: None,
            context: Some(SignatureHelpContext {
                is_retrigger: false,
                trigger_character: Some("(".to_string()),
                trigger_kind: Some(lsproto::SignatureHelpTriggerKind::TriggerCharacter),
            }),
        }],
    );
    done();
}
