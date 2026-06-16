#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_references_see_tag_in_ts() {
    let mut t = TestingT;
    run_test_find_references_see_tag_in_ts(&mut t);
}

fn run_test_find_references_see_tag_in_ts(t: &mut TestingT) {
    if should_skip_if_failing("TestFindReferencesSeeTagInTs") {
        return;
    }
    let content = r"function doStuffWithStuff/*1*/(stuff: { quantity: number }) {}

declare const stuff: { quantity: number };
/** @see {doStuffWithStuff} */
if (stuff.quantity) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
