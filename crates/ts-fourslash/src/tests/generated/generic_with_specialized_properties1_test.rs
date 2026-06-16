#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_with_specialized_properties1() {
    let mut t = TestingT;
    run_test_generic_with_specialized_properties1(&mut t);
}

fn run_test_generic_with_specialized_properties1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericWithSpecializedProperties1") {
        return;
    }
    let content = r"interface Foo<T> {
    x: Foo<string>;
    y: Foo<number>;
}
var f: Foo<number>;
var /*1*/xx = f.x;
var /*2*/yy = f.y;
var f2: Foo<string>;
var /*3*/x2 = f2.x;
var /*4*/y2 = f2.y;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var xx: Foo<string>", "");
    f.verify_quick_info_at(t, "2", "var yy: Foo<number>", "");
    f.verify_quick_info_at(t, "3", "var x2: Foo<string>", "");
    f.verify_quick_info_at(t, "4", "var y2: Foo<number>", "");
    done();
}
