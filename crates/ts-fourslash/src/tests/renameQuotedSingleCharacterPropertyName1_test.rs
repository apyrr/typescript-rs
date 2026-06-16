use crate::{new_fourslash, TestingT};

pub fn test_rename_quoted_single_character_property_name1(t: &mut TestingT) {
    let content = "\nconst obj = {\n  \"'\"/**/: 1,\n}\n";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_worker(t, &[], Some(""));
    done();
}

