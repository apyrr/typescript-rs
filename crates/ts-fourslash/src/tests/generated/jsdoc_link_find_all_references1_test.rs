#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_link_find_all_references1() {
    let mut t = TestingT;
    run_test_jsdoc_link_find_all_references1(&mut t);
}

fn run_test_jsdoc_link_find_all_references1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocLink_findAllReferences1") {
        return;
    }
    let content = r"interface A/**/ {}
/**
 * {@link A()} is ok
 */
declare const a: A";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["".to_string()]);
    done();
}
