#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_union_property() {
    let mut t = TestingT;
    run_test_find_all_refs_union_property(&mut t);
}

fn run_test_find_all_refs_union_property(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsUnionProperty") {
        return;
    }
    let content = r#"type T =
    | { /*t0*/type: "a", /*p0*/prop: number }
    | { /*t1*/type: "b", /*p1*/prop: string };
const tt: T = {
    /*t2*/type: "a",
    /*p2*/prop: 0,
};
declare const t: T;
if (t./*t3*/type === "a") {
    t./*t4*/type;
} else {
    t./*t5*/type;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "t0".to_string(),
            "t1".to_string(),
            "t3".to_string(),
            "t4".to_string(),
            "t5".to_string(),
            "t2".to_string(),
            "p0".to_string(),
            "p1".to_string(),
            "p2".to_string(),
        ],
    );
    done();
}
