#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_object_spread() {
    let mut t = TestingT;
    run_test_find_all_refs_for_object_spread(&mut t);
}

fn run_test_find_all_refs_for_object_spread(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface A1 { readonly /*0*/a: string };
interface A2 { /*1*/a?: number };
let a1: A1;
let a2: A2;
let a12 = { ...a1, ...a2 };
a12./*2*/a;
a1./*3*/a;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "0".to_string(),
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
        ],
    );
    done();
}
