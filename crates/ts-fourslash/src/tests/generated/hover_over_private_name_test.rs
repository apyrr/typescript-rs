#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_hover_over_private_name() {
    let mut t = TestingT;
    run_test_hover_over_private_name(&mut t);
}

fn run_test_hover_over_private_name(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class A {
    #f/*1*/oo = 3;
    #b/*2*/ar: number;
    #b/*3*/az = () => "hello";
    #q/*4*/ux(n: number): string {
        return "" + n;
    }
    static #staticF/*5*/oo = 3;
    static #staticB/*6*/ar: number;
    static #staticB/*7*/az = () => "hello";
    static #staticQ/*8*/ux(n: number): string {
        return "" + n;
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) A.#foo: number", "");
    f.verify_quick_info_at(t, "2", "(property) A.#bar: number", "");
    f.verify_quick_info_at(t, "3", "(property) A.#baz: () => string", "");
    f.verify_quick_info_at(t, "4", "(method) A.#qux(n: number): string", "");
    f.verify_quick_info_at(t, "5", "(property) A.#staticFoo: number", "");
    f.verify_quick_info_at(t, "6", "(property) A.#staticBar: number", "");
    f.verify_quick_info_at(t, "7", "(property) A.#staticBaz: () => string", "");
    f.verify_quick_info_at(t, "8", "(method) A.#staticQux(n: number): string", "");
    done();
}
