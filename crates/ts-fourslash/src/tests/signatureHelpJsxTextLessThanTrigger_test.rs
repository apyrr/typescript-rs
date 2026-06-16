use crate::{new_fourslash, SignatureHelpContext, TestingT};
use ts_lsproto as lsproto;

pub fn test_signature_help_jsx_text_less_than_trigger(t: &mut TestingT) {
    let content = r#"//@Filename: test.tsx
//@jsx: react
declare var React: any;
declare function Text(props: { children?: any }): any;

const text = () => {
	return <Text>/*m*/</Text>;
};"#;

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.go_to_marker(t, "m");
    f.insert(t, "<");
    f.verify_no_signature_help_with_context(
        t,
        Some(SignatureHelpContext {
            trigger_kind: Some(lsproto::SignatureHelpTriggerKind::TriggerCharacter),
            trigger_character: Some("<".to_string()),
            is_retrigger: false,
        }),
    );
    done();
}

