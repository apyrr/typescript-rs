use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

// Regression test for crash when hovering with verbosity on a namespace
// containing an interface that extends an intersection type alias.
// The base type resolves to an intersection (TypeFlagsIntersection),
// causing Type.Target() to panic with "Unhandled case in Type.Target".
// See: https://github.com/microsoft/typescript-go/issues/3466
pub fn test_quickinfo_verbosity_namespace_interface_heritage_intersection_crash(t: &mut TestingT) {
    let content = r#"
declare namespace NS/*1*/ {
    type Mixin = { a: string } & { b: number };
    interface Config extends Mixin {}
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("1".to_string(), vec![0, 1])]),
    );
    done();
}

