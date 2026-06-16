#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_type_parameter_in_type_alias1() {
    let mut t = TestingT;
    run_test_quick_info_for_type_parameter_in_type_alias1(&mut t);
}

fn run_test_quick_info_for_type_parameter_in_type_alias1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForTypeParameterInTypeAlias1") {
        return;
    }
    let content = r"type Ctor<AA> = new () => A/*1*/A;
type MixinCtor<AA> = new () => AA & { constructor: MixinCtor<A/*2*/A> };
type NestedCtor<AA> = new() => AA & (new () => AA & { constructor: NestedCtor<A/*3*/A> });
type Method<AA> = { method(): A/*4*/A };
type Construct<AA> = { new(): A/*5*/A };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(type parameter) AA in type Ctor<AA>", "");
    f.verify_quick_info_at(t, "2", "(type parameter) AA in type MixinCtor<AA>", "");
    f.verify_quick_info_at(t, "3", "(type parameter) AA in type NestedCtor<AA>", "");
    f.verify_quick_info_at(t, "4", "(type parameter) AA in type Method<AA>", "");
    f.verify_quick_info_at(t, "5", "(type parameter) AA in type Construct<AA>", "");
    done();
}
