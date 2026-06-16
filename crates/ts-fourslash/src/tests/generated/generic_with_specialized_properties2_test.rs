#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_with_specialized_properties2() {
    let mut t = TestingT;
    run_test_generic_with_specialized_properties2(&mut t);
}

fn run_test_generic_with_specialized_properties2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo<T> {
    y: Foo<number>;
    x: Foo<string>;
}
var f: Foo<string>;
var /*1*/x = f.x; 
var /*2*/y = f.y; 
var f2: Foo<number>;
var /*3*/x2 = f2.x; 
var /*4*/y2 = f2.y; ";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var x: Foo<string>", "");
    f.verify_quick_info_at(t, "2", "var y: Foo<number>", "");
    f.verify_quick_info_at(t, "3", "var x2: Foo<string>", "");
    f.verify_quick_info_at(t, "4", "var y2: Foo<number>", "");
    done();
}
