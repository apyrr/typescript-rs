#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_fundule_with_recursive_reference() {
    let mut t = TestingT;
    run_test_fundule_with_recursive_reference(&mut t);
}

fn run_test_fundule_with_recursive_reference(t: &mut TestingT) {
    if should_skip_if_failing("TestFunduleWithRecursiveReference") {
        return;
    }
    let content = r"namespace M {
    export function C() {}
    export namespace C {
    export var /**/C = M.C
  }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var M.C.C: typeof M.C", "");
    f.verify_no_errors();
    done();
}
