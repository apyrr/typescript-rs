#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_overload_quick_info() {
    let mut t = TestingT;
    run_test_overload_quick_info(&mut t);
}

fn run_test_overload_quick_info(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function Foo(a: string, b: number, c: boolean);
function Foo(a: any, name: string, age: number);
function Foo(fred: any[], name: string, age: number);
function Foo(fred: any[  ] , name: string[], age: number);
function Foo(fred: any[], name: string[], age: number[]);
function Foo(fred:         any, name: string[], age: number[]); // Extraneous spaces should get removed
function Foo(fred: any, name: boolean, age: number[]);
function Foo(dave: boolean, name: string);
function Foo(fred: any, mandy: {(): number}, age: number[]);    // Embedded interface will get converted to shorthand notation, () => 
function Foo(fred: any, name: string, age: { });
function Foo(fred: any, name: string, age: number[]);
function Foo(test: string, name, age: number);
function Foo();
function Foo(x?: any, y?: any, z?: any) {
}
Fo/**/o();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "function Foo(): any (+12 overloads)", "");
    done();
}
