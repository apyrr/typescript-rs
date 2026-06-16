#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_recursive_wrapped_type_parameters1() {
    let mut t = TestingT;
    run_test_recursive_wrapped_type_parameters1(&mut t);
}

fn run_test_recursive_wrapped_type_parameters1(t: &mut TestingT) {
    if should_skip_if_failing("TestRecursiveWrappedTypeParameters1") {
        return;
    }
    let content = r"interface I<T> {
	a: T;
	b: I<T>;
	c: I<I<T>>;
}
var x: I<number>;
var y/*1*/y = x.c.c.c.c.c.b;
var a/*2*/a = x.a;
var b/*3*/b = x.b;
var c/*4*/c = x.c;
var d/*5*/d = x.c.a;
var e/*6*/e = x.c.b;
var f/*7*/f = x.c.c; ";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var yy: I<I<I<I<I<I<number>>>>>>", "");
    f.verify_quick_info_at(t, "2", "var aa: number", "");
    f.verify_quick_info_at(t, "3", "var bb: I<number>", "");
    f.verify_quick_info_at(t, "4", "var cc: I<I<number>>", "");
    f.verify_quick_info_at(t, "5", "var dd: I<number>", "");
    f.verify_quick_info_at(t, "6", "var ee: I<I<number>>", "");
    f.verify_quick_info_at(t, "7", "var ff: I<I<I<number>>>", "");
    done();
}
