#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_interface_property_inference2() {
    let mut t = TestingT;
    run_test_generic_interface_property_inference2(&mut t);
}

fn run_test_generic_interface_property_inference2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
interface I {
    x: number;
}

var anInterface: I;
interface IG<T> {
    x: T;
}
var aGenericInterface: IG<number>;

class C<T> implements IG<T> {
    x: T;
}

interface Foo<T> {
    ofFooT: Foo<T>;
    ofFooFooNum: Foo<Foo<number>>; // should be error?
    ofIG: IG<T>;
    ofIG5: { x: Foo<T>; }; // same as ofIG3
    ofC1: C<T>;
}

var f: Foo<any>;
var f2: Foo<number>;
var f3: Foo<I>;
var f4: Foo<{ x: number }>;
var f5: Foo<Foo<number>>;

// T is any
var f_/*a1*/r4 = f.ofFooT;
var f_/*a2*/r7 = f.ofFooFooNum;
var f_/*a3*/r9 = f.ofIG;
var f_/*a5*/r13 = f.ofIG5;
var f_/*a7*/r17 = f.ofC1;

// T is number
var f2_/*b1*/r4 = f2.ofFooT;
var f2_/*b2*/r7 = f2.ofFooFooNum;
var f2_/*b3*/r9 = f2.ofIG;
var f2_/*b5*/r13 = f2.ofIG5;
var f2_/*b7*/r17 = f2.ofC1;

// T is I}
var f3_/*c1*/r4 = f3.ofFooT;
var f3_/*c2*/r7 = f3.ofFooFooNum;
var f3_/*c3*/r9 = f3.ofIG;
var f3_/*c5*/r13 = f3.ofIG5;
var f3_/*c7*/r17 = f3.ofC1;

// T is {x: number}
var f4_/*d1*/r4 = f4.ofFooT;
var f4_/*d2*/r7 = f4.ofFooFooNum;
var f4_/*d3*/r9 = f4.ofIG;
var f4_/*d5*/r13 = f4.ofIG5;
var f4_/*d7*/r17 = f4.ofC1;

// T is Foo<number>
var f5_/*e1*/r4 = f5.ofFooT;
var f5_/*e2*/r7 = f5.ofFooFooNum;
var f5_/*e3*/r9 = f5.ofIG;
var f5_/*e5*/r13 = f5.ofIG5;
var f5_/*e7*/r17 = f5.ofC1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_quick_info_at(t, "a1", "var f_r4: Foo<any>", "");
    f.verify_quick_info_at(t, "a2", "var f_r7: Foo<Foo<number>>", "");
    f.verify_quick_info_at(t, "a3", "var f_r9: IG<any>", "");
    f.verify_quick_info_at(t, "a5", "var f_r13: {\n    x: Foo<any>;\n}", "");
    f.verify_quick_info_at(t, "a7", "var f_r17: C<any>", "");
    f.verify_quick_info_at(t, "b1", "var f2_r4: Foo<number>", "");
    f.verify_quick_info_at(t, "b2", "var f2_r7: Foo<Foo<number>>", "");
    f.verify_quick_info_at(t, "b3", "var f2_r9: IG<number>", "");
    f.verify_quick_info_at(t, "b5", "var f2_r13: {\n    x: Foo<number>;\n}", "");
    f.verify_quick_info_at(t, "b7", "var f2_r17: C<number>", "");
    f.verify_quick_info_at(t, "c1", "var f3_r4: Foo<I>", "");
    f.verify_quick_info_at(t, "c2", "var f3_r7: Foo<Foo<number>>", "");
    f.verify_quick_info_at(t, "c3", "var f3_r9: IG<I>", "");
    f.verify_quick_info_at(t, "c5", "var f3_r13: {\n    x: Foo<I>;\n}", "");
    f.verify_quick_info_at(t, "c7", "var f3_r17: C<I>", "");
    f.verify_quick_info_at(t, "d1", "var f4_r4: Foo<{\n    x: number;\n}>", "");
    f.verify_quick_info_at(t, "d2", "var f4_r7: Foo<Foo<number>>", "");
    f.verify_quick_info_at(t, "d3", "var f4_r9: IG<{\n    x: number;\n}>", "");
    f.verify_quick_info_at(
        t,
        "d5",
        "var f4_r13: {\n    x: Foo<{\n        x: number;\n    }>;\n}",
        "",
    );
    f.verify_quick_info_at(t, "d7", "var f4_r17: C<{\n    x: number;\n}>", "");
    f.verify_quick_info_at(t, "e1", "var f5_r4: Foo<Foo<number>>", "");
    f.verify_quick_info_at(t, "e2", "var f5_r7: Foo<Foo<number>>", "");
    f.verify_quick_info_at(t, "e3", "var f5_r9: IG<Foo<number>>", "");
    f.verify_quick_info_at(t, "e5", "var f5_r13: {\n    x: Foo<Foo<number>>;\n}", "");
    f.verify_quick_info_at(t, "e7", "var f5_r17: C<Foo<number>>", "");
    done();
}
