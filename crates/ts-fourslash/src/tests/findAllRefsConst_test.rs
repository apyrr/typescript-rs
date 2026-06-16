use crate::{new_fourslash, TestingT};

pub fn test_find_all_refs_const(t: &mut TestingT) {
    let content = r#"// @Filename: a.ts
/**/const const
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}

