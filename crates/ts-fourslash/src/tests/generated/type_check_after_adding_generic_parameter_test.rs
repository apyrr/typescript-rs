#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_type_check_after_adding_generic_parameter() {
    let mut t = TestingT;
    run_test_type_check_after_adding_generic_parameter(&mut t);
}

fn run_test_type_check_after_adding_generic_parameter(t: &mut TestingT) {
    if should_skip_if_failing("TestTypeCheckAfterAddingGenericParameter") {
        return;
    }
    let content = r"function f<x, x>() { }
function f2<X, X>(b: X): X { return null; }
class C<X> {
    public f<x, x>() {}
f2<X>(b): X { return null; }
}

interface I<X, X> {
    f<X/*addTypeParam*/>();
    f2<X>(/*addParam*/a: X): X;
}
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "addParam");
    f.insert(t, ", X");
    f.go_to_marker(t, "addTypeParam");
    f.insert(t, ", X");
    done();
}
