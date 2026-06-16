use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

// Tests that expanded interface members are ordered correctly:
// index signatures, then construct signatures, then call signatures, then properties.
pub fn test_quickinfo_verbosity_interface_member_ordering(t: &mut TestingT) {
    let content = r#"
interface Callable/*1*/ {
    (x: string): boolean;
    new (x: string): Callable;
    [key: string]: any;
    name: string;
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("1".to_string(), vec![0, 1])]),
    );
    done();
}

