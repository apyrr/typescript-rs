use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

pub fn test_quickinfo_verbosity_empty_enum(t: &mut TestingT) {
    let content = r#"
enum Degree {}

declare const e/*0*/: Degree;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("0".to_string(), vec![0, 1])]),
    );
    done();
}

