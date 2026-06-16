#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_duplicate_function_implementation() {
    let mut t = TestingT;
    run_test_duplicate_function_implementation(&mut t);
}

fn run_test_duplicate_function_implementation(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface IFoo<T> {
    foo<T>(): T;
}
function foo<string>(/**/): string { return null; }
function foo<T>(x: T): T { return null; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "x: string");
    done();
}
