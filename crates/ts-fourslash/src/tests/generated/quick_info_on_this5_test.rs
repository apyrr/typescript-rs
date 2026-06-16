#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_this5() {
    let mut t = TestingT;
    run_test_quick_info_on_this5(&mut t);
}

fn run_test_quick_info_on_this5(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnThis5") {
        return;
    }
    let content = r"// @noImplicitThis: true
const foo = {
    num: 0,
    f() {
        type Y = typeof th/*1*/is;
        type Z = typeof th/*2*/is.num;
    },
    g(this: number) {
        type X = typeof th/*3*/is;
    }
}
class Foo {
    num = 0;
    f() {
        type Y = typeof th/*4*/is;
        type Z = typeof th/*5*/is.num;
    }
    g(this: number) {
        type X = typeof th/*6*/is;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
