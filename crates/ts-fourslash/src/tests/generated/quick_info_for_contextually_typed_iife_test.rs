#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_contextually_typed_iife() {
    let mut t = TestingT;
    run_test_quick_info_for_contextually_typed_iife(&mut t);
}

fn run_test_quick_info_for_contextually_typed_iife(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"(({ q/*1*/, qq/*2*/ }, x/*3*/, { p/*4*/ }) => {
    var s: number = q/*5*/;
    var t: number = qq/*6*/;
    var u: number = p/*7*/;
    var v: number = x/*8*/;
    return q; })({ q: 13, qq: 12 }, 1, { p: 14 });
((a/*9*/, b/*10*/, c/*11*/) => [a/*12*/,b/*13*/,c/*14*/])("foo", 101, false);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) q: number", "");
    f.verify_quick_info_at(t, "2", "(parameter) qq: number", "");
    f.verify_quick_info_at(t, "3", "(parameter) x: number", "");
    f.verify_quick_info_at(t, "4", "(parameter) p: number", "");
    f.verify_quick_info_at(t, "5", "(parameter) q: number", "");
    f.verify_quick_info_at(t, "6", "(parameter) qq: number", "");
    f.verify_quick_info_at(t, "7", "(parameter) p: number", "");
    f.verify_quick_info_at(t, "8", "(parameter) x: number", "");
    f.verify_quick_info_at(t, "9", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "10", "(parameter) b: number", "");
    f.verify_quick_info_at(t, "11", "(parameter) c: boolean", "");
    f.verify_quick_info_at(t, "12", "(parameter) a: string", "");
    f.verify_quick_info_at(t, "13", "(parameter) b: number", "");
    f.verify_quick_info_at(t, "14", "(parameter) c: boolean", "");
    done();
}
