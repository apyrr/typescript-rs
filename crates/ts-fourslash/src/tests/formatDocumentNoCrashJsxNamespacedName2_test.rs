use crate::{new_fourslash, TestingT};

pub fn test_format_document_no_crash_jsx_namespaced_name2(t: &mut TestingT) {
    let content = r#"// @Filename: /a.tsx
const x = <A my-ns:attr="val" />;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(t, "const x = <A my-ns:attr=\"val\" />;\n");
    done();
}

