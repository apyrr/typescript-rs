use crate::{new_fourslash, TestingT, VerifySignatureHelpOptions};

pub fn test_overload_on_const_call_signature(t: &mut TestingT) {
    let content = r#"var foo: {
    (name: string): string;
    (name: 'order'): string;
    (name: 'content'): string;
    (name: 'done'): string;
}
var /*2*/x = foo(/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("foo(name: 'order'): string".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 4,
        },
    );
    f.insert(t, "\"hi\"");
    f.verify_quick_info_at(t, "2", "var x: string", "");
    done();
}

