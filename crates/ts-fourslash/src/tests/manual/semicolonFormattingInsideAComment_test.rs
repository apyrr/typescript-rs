use crate::{new_fourslash, TestingT};

pub fn test_semicolon_formatting_inside_a_comment(t: &mut TestingT) {
    let content = r#"    ///**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, ";");
    f.verify_current_line_content(t, r#"   //;"#);
    done();
}

