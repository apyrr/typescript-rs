use crate::{new_fourslash, TestingT, VerifySignatureHelpOptions};

// Regression for getPossibleTypeArgumentsInfo (see PR #3222): when scanning backward through
// explicit type arguments, KindGreaterThanGreaterThanToken and KindGreaterThanGreaterThanGreaterThanToken
// must use += so the < / > balance accumulates across multiple closing-angle runs. A single `>`
// followed by `>>` (two separate tokens) must not be handled as if each `>>` reset the balance.
pub fn test_signature_help_nested_type_argument_gt_balance(t: &mut TestingT) {
    let content = r#"declare function f<T, U>(): void;
type A<T> = T;
type B<T> = T;
type C<T> = T;
f<A<B<C<number>>>, /*nested*/;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.go_to_marker(t, "nested");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("f<T, U>(): void".to_string()),
            parameter_name: Some("U".to_string()),
            parameter_span: Some("U".to_string()),
            parameter_count: Some(2),
            overloads_count: 0,
        },
    );
    done();
}

