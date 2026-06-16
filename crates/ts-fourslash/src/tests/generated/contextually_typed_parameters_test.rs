#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextually_typed_parameters() {
    let mut t = TestingT;
    run_test_contextually_typed_parameters(&mut t);
}

fn run_test_contextually_typed_parameters(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare function foo(cb: (this: any, x: number, y: string, z: boolean) => void): void;

foo(function(this, a, ...args) {
    a/*10*/;
    args/*11*/;
});

foo(function(this, a, b, ...args) {
    a/*20*/;
    b/*21*/;
    args/*22*/;
});

foo(function(this, a, b, c, ...args) {
    a/*30*/;
    b/*31*/;
    c/*32*/;
    args/*33*/;
});

foo(function(a, ...args) {
    a/*40*/;
    args/*41*/;
});

foo(function(a, b, ...args) {
    a/*50*/;
    b/*51*/;
    args/*52*/;
});

foo(function(a, b, c, ...args) {
    a/*60*/;
    b/*61*/;
    c/*62*/;
    args/*63*/;
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "10", "(parameter) a: number", "");
    f.verify_quick_info_at(t, "11", "(parameter) args: [y: string, z: boolean]", "");
    f.verify_quick_info_at(t, "20", "(parameter) a: number", "");
    f.verify_quick_info_at(t, "21", "(parameter) b: string", "");
    f.verify_quick_info_at(t, "22", "(parameter) args: [z: boolean]", "");
    f.verify_quick_info_at(t, "30", "(parameter) a: number", "");
    f.verify_quick_info_at(t, "31", "(parameter) b: string", "");
    f.verify_quick_info_at(t, "32", "(parameter) c: boolean", "");
    f.verify_quick_info_at(t, "33", "(parameter) args: []", "");
    f.verify_quick_info_at(t, "40", "(parameter) a: number", "");
    f.verify_quick_info_at(t, "41", "(parameter) args: [y: string, z: boolean]", "");
    f.verify_quick_info_at(t, "50", "(parameter) a: number", "");
    f.verify_quick_info_at(t, "51", "(parameter) b: string", "");
    f.verify_quick_info_at(t, "52", "(parameter) args: [z: boolean]", "");
    f.verify_quick_info_at(t, "60", "(parameter) a: number", "");
    f.verify_quick_info_at(t, "61", "(parameter) b: string", "");
    f.verify_quick_info_at(t, "62", "(parameter) c: boolean", "");
    f.verify_quick_info_at(t, "63", "(parameter) args: []", "");
    done();
}
