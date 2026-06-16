use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

// Tests expansion of a const enum with string initializers.
pub fn test_quickinfo_verbosity_const_enum(t: &mut TestingT) {
    let content = r#"
const enum Direction/*1*/ {
    Up = "UP",
    Down = "DOWN",
    Left = "LEFT",
    Right = "RIGHT",
}

enum NumericEnum/*2*/ {
    A,
    B = 10,
    C,
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("1".to_string(), vec![0, 1]), ("2".to_string(), vec![0, 1])]),
    );
    done();
}

