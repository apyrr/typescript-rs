#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_interface_used_as_generic_type_argument() {
    let mut t = TestingT;
    run_test_remove_interface_used_as_generic_type_argument(&mut t);
}

fn run_test_remove_interface_used_as_generic_type_argument(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/**/interface A { a: string; }
interface G<T, U> { }
var v1: G<A, C>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.delete_at_caret(t, 26);
    done();
}
