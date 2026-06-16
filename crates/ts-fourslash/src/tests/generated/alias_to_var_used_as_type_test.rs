#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_alias_to_var_used_as_type() {
    let mut t = TestingT;
    run_test_alias_to_var_used_as_type(&mut t);
}

fn run_test_alias_to_var_used_as_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/**/
namespace A {
export var X;
import Z = A.X;
var v: Z;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, " ");
    done();
}
