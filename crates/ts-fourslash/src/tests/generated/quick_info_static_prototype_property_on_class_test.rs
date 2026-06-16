#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_static_prototype_property_on_class() {
    let mut t = TestingT;
    run_test_quick_info_static_prototype_property_on_class(&mut t);
}

fn run_test_quick_info_static_prototype_property_on_class(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoStaticPrototypePropertyOnClass") {
        return;
    }
    let content = r"class c1 {
}
class c2<T> {
}
class c3 {
    constructor() {
    }
}
class c4 {
    constructor(param: string);
    constructor(param: number);
    constructor(param: any) {
    }
}
c1./*1*/prototype;
c2./*2*/prototype;
c3./*3*/prototype;
c4./*4*/prototype;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) c1.prototype: c1", "");
    f.verify_quick_info_at(t, "2", "(property) c2<T>.prototype: c2<any>", "");
    f.verify_quick_info_at(t, "3", "(property) c3.prototype: c3", "");
    f.verify_quick_info_at(t, "4", "(property) c4.prototype: c4", "");
    done();
}
