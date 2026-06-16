#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_interfaces_with_constraints1() {
    let mut t = TestingT;
    run_test_generic_interfaces_with_constraints1(&mut t);
}

fn run_test_generic_interfaces_with_constraints1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericInterfacesWithConstraints1") {
        return;
    }
    let content = r"interface A { a: string; }
interface B extends A { b: string; }
interface C extends B { c: string; }
interface G<T, U extends B> {
    x: T;
    y: U;
}
var v/*1*/1: G<A, C>;               // Ok
var v/*2*/2: G<{ a: string }, C>;   // Ok, equivalent to G<A, C>
var v/*3*/3: G<G<A, B>, C>;         // Ok";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var v1: G<A, C>", "");
    f.verify_quick_info_at(t, "2", "var v2: G<{\n    a: string;\n}, C>", "");
    f.verify_quick_info_at(t, "3", "var v3: G<G<A, B>, C>", "");
    done();
}
