use crate::{new_fourslash, TestingT};

pub fn test_hover_call_signature_documentation(t: &mut TestingT) {
    let content = r#"
type X = {
    /** Description of invoking. */
    (): string

    /** Description of constructor. */
    new (): number
}

declare const x: X

/*1*/x()
new /*2*/x()
"#;

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    f.verify_quick_info_at(t, "1", "const x: () => string", "Description of invoking.");
    f.verify_quick_info_at(
        t,
        "2",
        "const x: new () => number",
        "Description of constructor.",
    );
    done();
}

