#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextual_typing_generic_function1() {
    let mut t = TestingT;
    run_test_contextual_typing_generic_function1(&mut t);
}

fn run_test_contextual_typing_generic_function1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var obj: { f<T>(x: T): T } = { f: <S>(/*1*/x) => x };
var obj2: <T>(x: T) => T = <S>(/*2*/x) => x;

class C<T> {
    obj: <T>(x: T) => T
}
var c = new C();
c.obj = <S>(/*3*/x) => x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) x: any", "");
    f.verify_quick_info_at(t, "2", "(parameter) x: any", "");
    f.verify_quick_info_at(t, "3", "(parameter) x: any", "");
    done();
}
