use crate::{new_fourslash, TestingT};

pub fn test_format_document_no_crash_jsx_attr_unterminated_string(t: &mut TestingT) {
    let content = r#"// @Filename: /a.tsx
const x = <HangupButton customClass = 'ha
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(t, "const x = <HangupButton customClass= 'ha\n");
    done();
}

