#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_this2() {
    let mut t = TestingT;
    run_test_quick_info_on_this2(&mut t);
}

fn run_test_quick_info_on_this2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Bar<T> {
    public explicitThis(this: this) {
        console.log(th/*1*/is);
    }
    public explicitClass(this: Bar<T>) {
        console.log(thi/*2*/s);
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "this: this", "");
    f.verify_quick_info_at(t, "2", "this: Bar<T>", "");
    done();
}
