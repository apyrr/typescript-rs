#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_type_param_unrelated_to_arguments1() {
    let mut t = TestingT;
    run_test_generic_type_param_unrelated_to_arguments1(&mut t);
}

fn run_test_generic_type_param_unrelated_to_arguments1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo<T> {
    new (x: number): Foo<T>;
}
declare var f/*1*/1: Foo<number>;
var f/*2*/2: Foo<number>;
var f/*3*/3 = new Foo(3);
var f/*4*/4: Foo<number> = new Foo(3);
var f/*5*/5 = new Foo<number>(3);
var f/*6*/6: Foo<number> = new Foo<number>(3);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var f1: Foo<number>", "");
    f.verify_quick_info_at(t, "2", "var f2: Foo<number>", "");
    f.verify_quick_info_at(t, "3", "var f3: any", "");
    f.verify_quick_info_at(t, "4", "var f4: Foo<number>", "");
    f.verify_quick_info_at(t, "5", "var f5: any", "");
    f.verify_quick_info_at(t, "6", "var f6: Foo<number>", "");
    done();
}
