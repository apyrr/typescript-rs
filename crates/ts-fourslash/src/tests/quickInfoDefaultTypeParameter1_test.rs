use crate::{new_fourslash, TestingT};

pub fn test_quick_info_default_type_parameter1(t: &mut TestingT) {
    let content = r#"type /*1*/X</*2*/T = string> = T"#;

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "type X<T = string> = T", "");
    f.verify_quick_info_at(t, "2", "(type parameter) T in type X<T = string>", "");
    done();
}

