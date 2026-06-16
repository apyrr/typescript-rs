#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_getter_setter_no_crash1() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_getter_setter_no_crash1(&mut t);
}

fn run_test_quick_info_js_doc_getter_setter_no_crash1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class A implements A {
  get x(): string { return "" }
}
const e = new A()
e.x/*1*/

class B implements B {
  set x(v: string) {}
}
const f = new B()
f.x/*2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) A.x: string", "");
    f.verify_quick_info_at(t, "2", "(property) B.x: string", "");
    done();
}
