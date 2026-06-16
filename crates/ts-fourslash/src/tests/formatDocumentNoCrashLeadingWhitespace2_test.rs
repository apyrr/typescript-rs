use crate::{new_fourslash, TestingT};

pub fn test_format_document_no_crash_leading_whitespace2(t: &mut TestingT) {
    let content = " \n;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    done();
}

