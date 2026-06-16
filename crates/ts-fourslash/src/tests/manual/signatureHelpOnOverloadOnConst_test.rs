use crate::{new_fourslash, TestingT, VerifySignatureHelpOptions};

pub fn test_signature_help_on_overload_on_const(t: &mut TestingT) {
    let content = r#"function x1(x: 'hi');
function x1(y: 'bye');
function x1(z: string);
function x1(a: any) {
}

x1(''/*1*/);
x1('hi'/*2*/);
x1('bye'/*3*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("z".to_string()),
            parameter_span: Some("z: string".to_string()),
            parameter_count: None,
            overloads_count: 3,
        },
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("x".to_string()),
            parameter_span: Some("x: 'hi'".to_string()),
            parameter_count: None,
            overloads_count: 3,
        },
    );
    f.go_to_marker(t, "3");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: Some("y".to_string()),
            parameter_span: Some("y: 'bye'".to_string()),
            parameter_count: None,
            overloads_count: 3,
        },
    );
    done();
}

