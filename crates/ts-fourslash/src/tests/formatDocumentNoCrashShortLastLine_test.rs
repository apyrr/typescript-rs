use crate::{new_fourslash, TestingT};

pub fn test_format_document_no_crash_short_last_line(t: &mut TestingT) {
    let content = "type X = {\n\tb}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    done();
}

