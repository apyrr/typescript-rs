#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_interface_to_not_satisfy_constraint() {
    let mut t = TestingT;
    run_test_add_interface_to_not_satisfy_constraint(&mut t);
}

fn run_test_add_interface_to_not_satisfy_constraint(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface A {
	a: number;
}
/**/
interface C<T extends A> {
    x: T;
}

var v2: C<B>; // should not work";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "interface B { b: string; }");
    done();
}
