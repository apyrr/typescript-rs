use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

// Regression test for crash when hovering with verbosity on a namespace
// containing interfaces with generic heritage clauses.
// See: https://github.com/microsoft/typescript-go/pull/3454#issuecomment-4285883568
pub fn test_quickinfo_verbosity_namespace_interface_heritage_crash(t: &mut TestingT) {
    let content = r#"
declare namespace NS/*1*/ {
    interface Config extends Record<string, any> {}
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        BTreeMap::from([("1".to_string(), vec![0, 1])]),
    );
    done();
}

