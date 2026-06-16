#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_no_type_parameter_in_lhs() {
    let mut t = TestingT;
    run_test_no_type_parameter_in_lhs(&mut t);
}

fn run_test_no_type_parameter_in_lhs(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I<T> { }
class C<T> {}
var /*1*/i: I<any>;
var /*2*/c: C<I>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var i: I<any>", "");
    f.verify_quick_info_at(t, "2", "var c: C<any>", "");
    done();
}
