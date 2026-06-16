#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_with_specialized_properties3() {
    let mut t = TestingT;
    run_test_generic_with_specialized_properties3(&mut t);
}

fn run_test_generic_with_specialized_properties3(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericWithSpecializedProperties3") {
        return;
    }
    let content = r"interface Foo<T, U> {
    x: Foo<T, U>;
    y: Foo<U, U>;
}
var f: Foo<number, string>;
var /*1*/xx = f.x;
var /*2*/yy = f.y;
var f2: Foo<string, number>;
var /*3*/x2 = f2.x;
var /*4*/y2 = f2.y;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var xx: Foo<number, string>", "");
    f.verify_quick_info_at(t, "2", "var yy: Foo<string, string>", "");
    f.verify_quick_info_at(t, "3", "var x2: Foo<string, number>", "");
    f.verify_quick_info_at(t, "4", "var y2: Foo<number, number>", "");
    done();
}
