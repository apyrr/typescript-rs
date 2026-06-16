#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_recursive_generics2() {
    let mut t = TestingT;
    run_test_recursive_generics2(&mut t);
}

fn run_test_recursive_generics2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class S18<B, B, A, B> extends S18<A[], { S19: A; (): A }[]> { }
/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "(new S18()).S18 = 0;");
    done();
}
