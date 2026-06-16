#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_this3() {
    let mut t = TestingT;
    run_test_quick_info_on_this3(&mut t);
}

fn run_test_quick_info_on_this3(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnThis3") {
        return;
    }
    let content = r"interface Restricted {
    n: number;
}
function implicitAny(x: number): void {
    return th/*1*/is;
}
function explicitVoid(th/*2*/is: void, x: number): void {
    return th/*3*/is;
}
function explicitInterface(th/*4*/is: Restricted): void {
    console.log(thi/*5*/s);
}
function explicitLiteral(th/*6*/is: { n: number }): void {
    console.log(th/*7*/is);
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "any", "");
    f.verify_quick_info_at(t, "2", "(parameter) this: void", "");
    f.verify_quick_info_at(t, "3", "this: void", "");
    f.verify_quick_info_at(t, "4", "(parameter) this: Restricted", "");
    f.verify_quick_info_at(t, "5", "this: Restricted", "");
    f.verify_quick_info_at(t, "6", "(parameter) this: {\n    n: number;\n}", "");
    f.verify_quick_info_at(t, "7", "this: {\n    n: number;\n}", "");
    done();
}
