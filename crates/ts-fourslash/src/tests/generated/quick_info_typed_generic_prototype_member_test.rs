#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_typed_generic_prototype_member() {
    let mut t = TestingT;
    run_test_quick_info_typed_generic_prototype_member(&mut t);
}

fn run_test_quick_info_typed_generic_prototype_member(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C<T> {
   foo(x: T) { }
}
var /*1*/x = new C<any>(); // Quick Info for x is C<any>
var /*2*/y = C.prototype; // Quick Info for y is C<{}>";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var x: C<any>", "");
    f.verify_quick_info_at(t, "2", "var y: C<any>", "");
    done();
}
