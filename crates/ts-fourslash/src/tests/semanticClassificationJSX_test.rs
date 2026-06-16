use crate::{new_fourslash, SemanticToken, TestingT};

pub fn test_semantic_classification_jsx(t: &mut TestingT) {
    let content = r#"// @Filename: /a.tsx
const Component = () => <div>Hello</div>;
const afterJSX = 42;
const alsoAfterJSX = "test";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.tsx");
    f.verify_semantic_tokens(
        t,
        &[
            semantic_token("function.declaration.readonly", "Component"),
            semantic_token("variable.declaration.readonly", "afterJSX"),
            semantic_token("variable.declaration.readonly", "alsoAfterJSX"),
        ],
    );
    done();
}

fn semantic_token(type_: &str, text: &str) -> SemanticToken {
    SemanticToken {
        type_: type_.to_string(),
        text: text.to_string(),
    }
}

