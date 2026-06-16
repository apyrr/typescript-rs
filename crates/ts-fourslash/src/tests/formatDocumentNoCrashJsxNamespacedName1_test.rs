use crate::{new_fourslash, TestingT};

pub fn test_format_document_no_crash_jsx_namespaced_name1(t: &mut TestingT) {
    let content = r#"// @Filename: /a.tsx
const x = <foo:bar />;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(t, "const x = <foo:bar />;\n");
    done();
}

