use crate::{new_fourslash, TestingT};

pub fn test_signature_help_anonymous_type(t: &mut TestingT) {
    let content = r#"const comparers: Array<(a: any, b: any) => boolean> = [];

comparers.push((a,/**/ b) => true);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}

