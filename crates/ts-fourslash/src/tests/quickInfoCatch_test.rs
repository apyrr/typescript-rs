use crate::{new_fourslash, TestingT};

pub fn test_quick_catch_info(t: &mut TestingT) {
    let content = r#"try {} catch(/*1*/error) {}"#;

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var error: unknown", "");
    done();
}

