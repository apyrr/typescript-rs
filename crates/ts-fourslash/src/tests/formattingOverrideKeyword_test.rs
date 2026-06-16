use crate::{new_fourslash, TestingT};

pub fn test_formatting_override_keyword(t: &mut TestingT) {
    let content = r#"class MyClass {
  override     myMethod() { };/*1*/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, r#"    override myMethod() { };"#);
    done();
}

