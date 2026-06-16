#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_array_call_and_construct_typings() {
    let mut t = TestingT;
    run_test_array_call_and_construct_typings(&mut t);
}

fn run_test_array_call_and_construct_typings(t: &mut TestingT) {
    if should_skip_if_failing("TestArrayCallAndConstructTypings") {
        return;
    }
    let content = r#"var a/*1*/1 = new Array();
var a/*2*/2 = new Array(1);
var a/*3*/3 = new Array<boolean>();
var a/*4*/4 = new Array<boolean>(1);
var a/*5*/5 = new Array("s");
var a/*6*/6 = Array();
var a/*7*/7 = Array(1);
var a/*8*/8 = Array<boolean>();
var a/*9*/9 = Array<boolean>(1);
var a/*10*/10 = Array("s");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var a1: any[]", "");
    f.verify_quick_info_at(t, "2", "var a2: any[]", "");
    f.verify_quick_info_at(t, "3", "var a3: boolean[]", "");
    f.verify_quick_info_at(t, "4", "var a4: boolean[]", "");
    f.verify_quick_info_at(t, "5", "var a5: string[]", "");
    f.verify_quick_info_at(t, "6", "var a6: any[]", "");
    f.verify_quick_info_at(t, "7", "var a7: any[]", "");
    f.verify_quick_info_at(t, "8", "var a8: boolean[]", "");
    f.verify_quick_info_at(t, "9", "var a9: boolean[]", "");
    f.verify_quick_info_at(t, "10", "var a10: string[]", "");
    done();
}
