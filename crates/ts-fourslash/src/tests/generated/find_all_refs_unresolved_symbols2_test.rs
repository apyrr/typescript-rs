#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_unresolved_symbols2() {
    let mut t = TestingT;
    run_test_find_all_refs_unresolved_symbols2(&mut t);
}

fn run_test_find_all_refs_unresolved_symbols2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"import { /*a0*/Bar } from "does-not-exist";

let a: /*a1*/Bar;
let b: /*a2*/Bar<string>;
let c: /*a3*/Bar<string, number>;
let d: /*a4*/Bar./*b0*/X;
let e: /*a5*/Bar./*b1*/X<string>;
let f: /*a6*/Bar./*c0*/X./*d0*/Y;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "a0".to_string(),
            "a1".to_string(),
            "a2".to_string(),
            "a3".to_string(),
            "a4".to_string(),
            "a5".to_string(),
            "a6".to_string(),
            "b0".to_string(),
            "b1".to_string(),
            "c0".to_string(),
            "d0".to_string(),
        ],
    );
    done();
}
