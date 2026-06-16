use crate::{new_fourslash, TestingT};

pub fn test_quick_info_function(t: &mut TestingT) {
    let content = r#"/**/function foo() { return "hi"; }"#;

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "function foo(): string", "");
    done();
}

