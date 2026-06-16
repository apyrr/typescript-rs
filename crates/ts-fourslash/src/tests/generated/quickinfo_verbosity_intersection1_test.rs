#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_verbosity_intersection1() {
    let mut t = TestingT;
    run_test_quickinfo_verbosity_intersection1(&mut t);
}

fn run_test_quickinfo_verbosity_intersection1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoVerbosityIntersection1") {
        return;
    }
    let content = r#"{
    type Foo = { a: "a" | "c" };
    type Bar = { a: "a" | "b" };
    const obj/*o1*/: Foo & Bar = { a: "a" };
}
{
    type Foo = { a: "c" };
    type Bar = { a: "b" };
    const obj/*o2*/: Foo & Bar = { a: "" };
}
{
    type Foo = { a: "c" };
    type Bar = { a: "b" };
    type Never = Foo & Bar;
    const obj/*o3*/: Never = { a: "" };
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover_with_verbosity_by_marker(
        t,
        std::collections::BTreeMap::from([
            ("o1".to_string(), vec![0, 1]),
            ("o2".to_string(), vec![0]),
            ("o3".to_string(), vec![0]),
        ]),
    );
    done();
}
