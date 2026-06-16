#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_constructor_quick_info() {
    let mut t = TestingT;
    run_test_constructor_quick_info(&mut t);
}

fn run_test_constructor_quick_info(t: &mut TestingT) {
    if should_skip_if_failing("TestConstructorQuickInfo") {
        return;
    }
    let content = r"class SS<T>{}

var x/*1*/1 = new SS<number>();
var x/*2*/2 = new SS();
var x/*3*/3 = new SS;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var x1: SS<number>", "");
    f.verify_quick_info_at(t, "2", "var x2: SS<unknown>", "");
    f.verify_quick_info_at(t, "3", "var x3: SS<unknown>", "");
    done();
}
