#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_unresolved_symbols1() {
    let mut t = TestingT;
    run_test_find_all_refs_unresolved_symbols1(&mut t);
}

fn run_test_find_all_refs_unresolved_symbols1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"let a: /*a0*/Bar;
let b: /*a1*/Bar<string>;
let c: /*a2*/Bar<string, number>;
let d: /*b0*/Bar./*c0*/X;
let e: /*b1*/Bar./*c1*/X<string>;
let f: /*b2*/Bar./*d0*/X./*e0*/Y;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "a0".to_string(),
            "a1".to_string(),
            "a2".to_string(),
            "b0".to_string(),
            "b1".to_string(),
            "b2".to_string(),
            "c0".to_string(),
            "c1".to_string(),
            "d0".to_string(),
            "e0".to_string(),
        ],
    );
    done();
}
