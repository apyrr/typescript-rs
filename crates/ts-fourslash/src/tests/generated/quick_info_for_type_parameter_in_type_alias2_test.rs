#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_type_parameter_in_type_alias2() {
    let mut t = TestingT;
    run_test_quick_info_for_type_parameter_in_type_alias2(&mut t);
}

fn run_test_quick_info_for_type_parameter_in_type_alias2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForTypeParameterInTypeAlias2") {
        return;
    }
    let content = r"type Call<AA> = { (): A/*1*/A };
type Index<AA> = {[foo: string]: A/*2*/A};
type GenericMethod<AA> = { method<BB>(): A/*3*/A & B/*4*/B }
type Nesting<TT> = { method<UU>(): new <WW>() => T/*5*/T & U/*6*/U & W/*7*/W };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(type parameter) AA in type Call<AA>", "");
    f.verify_quick_info_at(t, "2", "(type parameter) AA in type Index<AA>", "");
    f.verify_quick_info_at(t, "3", "(type parameter) AA in type GenericMethod<AA>", "");
    f.verify_quick_info_at(t, "4", "(type parameter) BB in method<BB>(): AA & BB", "");
    f.verify_quick_info_at(t, "5", "(type parameter) TT in type Nesting<TT>", "");
    f.verify_quick_info_at(
        t,
        "6",
        "(type parameter) UU in method<UU>(): new <WW>() => TT & UU & WW",
        "",
    );
    f.verify_quick_info_at(t, "7", "(type parameter) WW in <WW>(): TT & UU & WW", "");
    done();
}
