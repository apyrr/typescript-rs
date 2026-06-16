#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_typedef_tag_semantic_meaning1() {
    let mut t = TestingT;
    run_test_jsdoc_typedef_tag_semantic_meaning1(&mut t);
}

fn run_test_jsdoc_typedef_tag_semantic_meaning1(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocTypedefTagSemanticMeaning1") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: a.js
/** @typedef {number} */
/*1*/const /*2*/T = 1;
/** @type {/*3*/T} */
const n = /*4*/T;";
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
