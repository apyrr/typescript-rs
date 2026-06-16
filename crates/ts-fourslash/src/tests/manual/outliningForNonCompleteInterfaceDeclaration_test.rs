use crate::{new_fourslash, TestingT};

pub fn test_outlining_for_non_complete_interface_declaration(t: &mut TestingT) {
    let content = r#"interface I"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}

