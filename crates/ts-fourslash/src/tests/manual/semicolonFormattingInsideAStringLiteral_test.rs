use crate::{new_fourslash, TestingT};

pub fn test_semicolon_formatting_inside_a_string_literal(t: &mut TestingT) {
    let content = r#"    var x = "string/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, ";");
    f.verify_current_line_content(t, r#"   var x = "string;"#);
    done();
}

