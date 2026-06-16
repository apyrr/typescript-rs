#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_insert_public_before_setter() {
    let mut t = TestingT;
    run_test_insert_public_before_setter(&mut t);
}

fn run_test_insert_public_before_setter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C {
    /**/set Bar(bar:string) {}
}
var o2 = { set Foo(val:number) { } };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "public ");
    done();
}
