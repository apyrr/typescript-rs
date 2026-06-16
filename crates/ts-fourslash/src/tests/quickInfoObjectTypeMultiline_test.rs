use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

pub fn test_quick_info_object_type_multiline(t: &mut TestingT) {
    let content = r#"
type X/*1*/ = {
    a: number
    b: string
    c: C
}
type C = {}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("1".to_string(), vec![0, 1])]),
    );
    done();
}

