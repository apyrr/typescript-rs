#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_map_typing1() {
    let mut t = TestingT;
    run_test_generic_map_typing1(&mut t);
}

fn run_test_generic_map_typing1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericMapTyping1") {
        return;
    }
    let content = r"// @strict: false
interface Iterator_<T, U> {
    (value: T, index: any, list: any): U;
}
interface WrappedArray<T> {
    map<U>(iterator: Iterator_<T, U>, context?: any): U[];
}
interface Underscore {
    <T>(list: T[]): WrappedArray<T>;
    map<T, U>(list: T[], iterator: Iterator_<T, U>, context?: any): U[];
}
declare var _: Underscore;
var aa: string[];
var b/*1*/b = _.map(aa, x/*7*/x => xx.length);    // should be number[]
var c/*2*/c = _(aa).map(x/*8*/x => xx.length);    // should be number[]
var d/*3*/d = aa.map(xx => x/*9*/x.length);       // should be number[]
var aaa: any[];
var b/*4*/bb = _.map(aaa, xx => xx.length); // should be any[]
var c/*5*/cc = _(aaa).map(xx => xx.length);  // Should not error, should be any[]
var d/*6*/dd = aaa.map(xx => xx.length);     // should not error, should be any[]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_quick_info_at(t, "1", "var bb: number[]", "");
    f.verify_quick_info_at(t, "2", "var cc: number[]", "");
    f.verify_quick_info_at(t, "3", "var dd: number[]", "");
    f.verify_quick_info_at(t, "4", "var bbb: any[]", "");
    f.verify_quick_info_at(t, "5", "var ccc: any[]", "");
    f.verify_quick_info_at(t, "6", "var ddd: any[]", "");
    f.verify_quick_info_at(t, "7", "(parameter) xx: string", "");
    f.verify_quick_info_at(t, "8", "(parameter) xx: string", "");
    f.verify_quick_info_at(t, "9", "(parameter) xx: string", "");
    done();
}
