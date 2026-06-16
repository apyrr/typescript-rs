#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_method_param() {
    let mut t = TestingT;
    run_test_generic_method_param(&mut t);
}

fn run_test_generic_method_param(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class C<T> {
    /*1*/
}
/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.go_to_marker(t, "1");
    f.insert_line(t, "constructor(){}");
    f.insert_line(t, "foo(a: T) {");
    f.insert_line(t, "    return a;");
    f.insert_line(t, "}");
    f.verify_no_errors();
    f.go_to_marker(t, "2");
    f.insert_line(t, "var x = new C<number>();");
    f.insert_line(t, "var y: number = x.foo(5);");
    f.verify_no_errors();
    done();
}
