use crate::{new_fourslash, SemanticToken, TestingT};

pub fn test_semantic_modern_classification_functions(t: &mut TestingT) {
    let content = r#"function foo(p1) {
  return foo(Math.abs(p1))
}
`/${window.location}`.split("/").forEach(s => foo(s));"#;
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_semantic_tokens(
        t,
        &[
            semantic_token("function.declaration", "foo"),
            semantic_token("parameter.declaration", "p1"),
            semantic_token("function", "foo"),
            semantic_token("variable.defaultLibrary", "Math"),
            semantic_token("method.defaultLibrary", "abs"),
            semantic_token("parameter", "p1"),
            semantic_token("variable.defaultLibrary", "window"),
            semantic_token("property.defaultLibrary", "location"),
            semantic_token("method.defaultLibrary", "split"),
            semantic_token("method.defaultLibrary", "forEach"),
            semantic_token("parameter.declaration", "s"),
            semantic_token("function", "foo"),
            semantic_token("parameter", "s"),
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

