#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_throws_tag() {
    let mut t = TestingT;
    run_test_quick_info_throws_tag(&mut t);
}

fn run_test_quick_info_throws_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoThrowsTag") {
        return;
    }
    let content = r"class E extends Error {}

/**
 * @throws {E}
 */
function f1() {}

/**
 * @throws {E} description
 */
function f2() {}

/**
 * @throws description
 */
function f3() {}
f1/*1*/()
f2/*2*/()
f3/*3*/()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_hover(t, &[]);
    done();
}
