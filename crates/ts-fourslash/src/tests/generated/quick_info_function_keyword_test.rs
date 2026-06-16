#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_function_keyword() {
    let mut t = TestingT;
    run_test_quick_info_function_keyword(&mut t);
}

fn run_test_quick_info_function_keyword(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"[1].forEach(fu/*1*/nction() {});
[1].map(x =/*2*/> x + 1);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(local function)(): void", "");
    f.verify_quick_info_at(t, "2", "function(x: number): number", "");
    done();
}
