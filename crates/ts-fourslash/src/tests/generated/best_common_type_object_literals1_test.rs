#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_best_common_type_object_literals1() {
    let mut t = TestingT;
    run_test_best_common_type_object_literals1(&mut t);
}

fn run_test_best_common_type_object_literals1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var a = { name: 'bob', age: 18 };
var b = { name: 'jim', age: 20 };
var /*1*/c = [a, b];
var a1 = { name: 'bob', age: 18 };
var b1 = { name: 'jim', age: 20, dob: new Date() };
var /*2*/c1 = [a1, b1];
var a2 = { name: 'bob', age: 18, address: 'springfield' };
var b2 = { name: 'jim', age: 20, dob: new Date() };
var /*3*/c2 = [a2, b2];
interface I {
    name: string;
    age: number;
}
var i: I;
var /*4*/c3 = [i, a];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "4", "var c3: I[]", "");
    f.verify_quick_info_at(
        t,
        "1",
        "var c: {\n    name: string;\n    age: number;\n}[]",
        "",
    );
    f.verify_quick_info_at(
        t,
        "2",
        "var c1: {\n    name: string;\n    age: number;\n}[]",
        "",
    );
    f.verify_quick_info_at(t, "3", "var c2: ({\n    name: string;\n    age: number;\n    address: string;\n} | {\n    name: string;\n    age: number;\n    dob: Date;\n})[]", "");
    f.verify_quick_info_at(t, "4", "var c3: I[]", "");
    done();
}
