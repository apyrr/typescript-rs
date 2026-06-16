#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_indexed_access_types() {
    let mut t = TestingT;
    run_test_find_all_refs_indexed_access_types(&mut t);
}

fn run_test_find_all_refs_indexed_access_types(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsIndexedAccessTypes") {
        return;
    }
    let content = r#"interface I {
    /*1*/0: number;
    /*2*/s: string;
}
interface J {
    a: I[/*3*/0],
    b: I["/*4*/s"],
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
    );
    done();
}
