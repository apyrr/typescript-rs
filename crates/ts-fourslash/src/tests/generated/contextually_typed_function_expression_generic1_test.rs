#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextually_typed_function_expression_generic1() {
    let mut t = TestingT;
    run_test_contextually_typed_function_expression_generic1(&mut t);
}

fn run_test_contextually_typed_function_expression_generic1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Comparable<T> {
   compareTo(other: T): T;
}
interface Comparer {
   <T extends Comparable<T>>(x: T, y: T): T;
}
var max2: Comparer = (x/*1*/x, y/*2*/y) => { return x/*3*/x.compareTo(y/*4*/y) };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) xx: T extends Comparable<T>", "");
    f.verify_quick_info_at(t, "2", "(parameter) yy: T extends Comparable<T>", "");
    f.verify_quick_info_at(t, "3", "(parameter) xx: T extends Comparable<T>", "");
    f.verify_quick_info_at(t, "4", "(parameter) yy: T extends Comparable<T>", "");
    done();
}
