#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_mapped_type() {
    let mut t = TestingT;
    run_test_find_all_refs_mapped_type(&mut t);
}

fn run_test_find_all_refs_mapped_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface T { /*1*/a: number; }
type U = { readonly [K in keyof T]?: string };
declare const t: T;
t./*2*/a;
declare const u: U;
u./*3*/a;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
