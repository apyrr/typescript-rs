#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_is_definition() {
    let mut t = TestingT;
    run_test_find_all_refs_is_definition(&mut t);
}

fn run_test_find_all_refs_is_definition(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsIsDefinition") {
        return;
    }
    let content = r"declare function foo(a: number): number;
declare function foo(a: string): string;
declare function foo/*1*/(a: string | number): string | number;

function foon(a: number): number;
function foon(a: string): string;
function foon/*2*/(a: string | number): string | number {
    return a
}

foo; foon;

export const bar/*3*/ = 123;
console.log({ bar });

interface IFoo {
    foo/*4*/(): void;
}
class Foo implements IFoo {
    constructor(n: number)
    constructor()
    /*5*/constructor(n: number?) { }
    foo/*6*/(): void { }
    static init() { return new this() }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
        ],
    );
    done();
}
