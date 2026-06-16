#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_generic_prototype_member() {
    let mut t = TestingT;
    run_test_quick_info_for_generic_prototype_member(&mut t);
}

fn run_test_quick_info_for_generic_prototype_member(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C<T> {
   foo(x: T) { }
}
var x = new /*1*/C<any>();
var y = C.proto/*2*/type;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "constructor C<any>(): C<any>", "");
    f.verify_quick_info_at(t, "2", "(property) C<T>.prototype: C<any>", "");
    done();
}
