use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

// Tests expansion of a class+interface merge (interface should be filtered when hovering value).
pub fn test_quickinfo_verbosity_class_interface_merge(t: &mut TestingT) {
    let content = r#"
declare class Foo/*1*/ {
    x: number;
}
declare interface Foo {
    y: string;
}
const f: Foo/*2*/ = { x: 1, y: "hello" };
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("1".to_string(), vec![0, 1]), ("2".to_string(), vec![0, 1])]),
    );
    done();
}

